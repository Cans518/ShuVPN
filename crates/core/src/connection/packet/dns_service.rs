use crate::connection::{model::VpnFailure, session::Shared, tunnel};
use crate::routing::{
    authorization,
    dns::{Resolution, Source},
    model::normalize_domain,
};
use hickory_proto::{
    op::{Message, MessageType, OpCode, ResponseCode},
    rr::{DNSClass, RecordType},
};
use std::sync::atomic::Ordering;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn failure(message: &str) -> VpnFailure {
    VpnFailure::new("dns", message)
}

pub(super) fn error_response(request: &Message, code: ResponseCode) -> Message {
    let mut response = Message::new();
    response
        .set_id(request.id())
        .set_message_type(MessageType::Response)
        .set_op_code(request.op_code())
        .set_recursion_desired(request.recursion_desired())
        .set_recursion_available(true)
        .set_response_code(code);
    for question in request.queries() {
        response.add_query(question.clone());
    }
    response
}

pub(super) async fn answer(shared: &Shared, request: &Message) -> Result<Message, VpnFailure> {
    if request.message_type() != MessageType::Query
        || request.op_code() != OpCode::Query
        || request.queries().len() != 1
        || request.queries()[0].query_class() != DNSClass::IN
    {
        return Ok(error_response(request, ResponseCode::NotImp));
    }
    let question = &request.queries()[0];
    let raw = question.name().to_utf8();
    let host = if raw == "." {
        raw
    } else {
        normalize_domain(&raw).map_err(|error| failure(&error.message))?
    };
    let campus = host == "shu.edu.cn"
        || host.ends_with(".shu.edu.cn")
        || shared.plan.routes.iter().any(|route| {
            route.host.parse::<std::net::IpAddr>().is_err()
                && route.host.parse::<ipnet::IpNet>().is_err()
                && authorization::address_range(&route.host).is_none()
                && authorization::domain_match(&route.host, &host)
        });
    let (version, epoch) = {
        let routing = shared
            .routing
            .read()
            .map_err(|_| failure("无法读取规则版本"))?;
        (
            routing
                .snapshot()
                .map_err(|error| failure(&error.message))?
                .info
                .version,
            shared
                .dns
                .epoch()
                .map_err(|error| failure(&error.message))?,
        )
    };
    let mut random = [0; 2];
    getrandom::getrandom(&mut random).map_err(|_| failure("无法生成 DNS 标识"))?;
    let mut query = request.clone();
    query.set_id(u16::from_be_bytes(random));
    let (mut response, source) = if campus {
        let (response, server) = tunnel::exchange_campus_dns(shared, &query).await?;
        (response, Source::Campus { server })
    } else {
        let _gate = crate::network::read_gate().await;
        let egress =
            crate::network::current()?.ok_or_else(|| failure("完整 DNS 请求没有物理出口保护"))?;
        shared.dns.system_lookups.fetch_add(1, Ordering::Relaxed);
        (
            crate::network::dns::exchange(&egress, &query).await?,
            Source::System,
        )
    };
    let current = shared
        .routing
        .read()
        .map_err(|_| failure("无法读取规则版本"))?
        .snapshot()
        .map_err(|error| failure(&error.message))?
        .info
        .version;
    if current != version
        || shared
            .dns
            .epoch()
            .map_err(|error| failure(&error.message))?
            != epoch
    {
        return Err(failure("DNS 回答所属配置已被替换"));
    }
    if response.response_code() == ResponseCode::NoError
        && host != "."
        && matches!(question.query_type(), RecordType::A | RecordType::AAAA)
    {
        let mut answer = crate::routing::dns::parse_response(&query, &response)
            .map_err(|error| failure(&error.message))?;
        let captured = shared
            .full_capture_routes
            .read()
            .map_err(|_| failure("无法读取捕获范围"))?
            .clone();
        answer
            .addresses
            .retain(|ip| captured.iter().any(|network| network.contains(ip)));
        if !answer.addresses.is_empty()
            && answer.ttl != Some(0)
            && !answer.expired(std::time::Instant::now())
        {
            let resolution = Resolution::new(host, source, version, vec![answer], vec![]);
            let resolution = shared
                .dns
                .insert_family(&resolution, epoch, question.query_type())
                .map_err(|error| failure(&error.message))?;
            crate::connection::capture::advertise(shared, resolution, epoch).await?;
        }
    }
    let current = shared
        .routing
        .read()
        .map_err(|_| failure("无法读取规则版本"))?
        .snapshot()
        .map_err(|error| failure(&error.message))?
        .info
        .version;
    if current != version
        || shared
            .dns
            .epoch()
            .map_err(|error| failure(&error.message))?
            != epoch
    {
        return Err(failure("DNS 回答发布前配置已变化"));
    }
    response.set_id(request.id());
    Ok(response)
}

pub(super) async fn tcp(
    shared: &Shared,
    mut stream: tokio::io::DuplexStream,
) -> Result<(), VpnFailure> {
    loop {
        let size = match stream.read_u16().await {
            Ok(size) => usize::from(size),
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        if size < 12 {
            return Err(failure("TCP DNS 请求过短"));
        }
        let mut bytes = vec![0; size];
        stream.read_exact(&mut bytes).await?;
        let request = Message::from_vec(&bytes).map_err(|_| failure("TCP DNS 请求无效"))?;
        let response = match answer(shared, &request).await {
            Ok(response) => response,
            Err(_) => error_response(&request, ResponseCode::ServFail),
        };
        let bytes = response
            .to_vec()
            .map_err(|_| failure("TCP DNS 响应编码失败"))?;
        if bytes.len() > u16::MAX as usize {
            return Err(failure("TCP DNS 响应过大"));
        }
        stream.write_u16(bytes.len() as u16).await?;
        stream.write_all(&bytes).await?;
        stream.flush().await?;
    }
}
