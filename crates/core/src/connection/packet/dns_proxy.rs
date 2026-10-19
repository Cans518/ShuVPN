use super::codec::{self, Tuple};
use crate::connection::{model::VpnFailure, session::Shared, tunnel};
use hickory_proto::{
    op::{Message, MessageType, OpCode, ResponseCode},
    rr::{
        rdata::{A, AAAA},
        DNSClass, RData, Record, RecordType,
    },
};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::mpsc;

pub(super) async fn serve(
    shared: &Shared,
    tuple: Tuple,
    mut incoming: mpsc::Receiver<Vec<u8>>,
    outgoing: mpsc::Sender<Vec<u8>>,
) -> Result<(), VpnFailure> {
    while let Ok(Some(packet)) =
        tokio::time::timeout(Duration::from_secs(30), incoming.recv()).await
    {
        let datagram = codec::parse(&packet)?;
        let request = Message::from_vec(datagram.payload)
            .map_err(|_| VpnFailure::new("dns", "TUN DNS 请求编码无效"))?;
        if request.message_type() != MessageType::Query
            || request.op_code() != OpCode::Query
            || request.queries().len() != 1
            || request.queries()[0].query_class() != DNSClass::IN
        {
            return Err(VpnFailure::new("dns", "TUN DNS 仅接受单一 IN 查询"));
        }
        let question = &request.queries()[0];
        let mut response = Message::new();
        response
            .set_id(request.id())
            .set_message_type(MessageType::Response)
            .set_op_code(OpCode::Query)
            .set_recursion_desired(request.recursion_desired())
            .set_recursion_available(true)
            .add_query(question.clone());
        if shared.dns_capture_full.load(Ordering::Acquire) {
            response = match super::dns_service::answer(shared, &request).await {
                Ok(response) => response,
                Err(_) => super::dns_service::error_response(&request, ResponseCode::ServFail),
            };
        } else if !matches!(question.query_type(), RecordType::A | RecordType::AAAA) {
            response.set_response_code(ResponseCode::NotImp);
        } else {
            let host = question.name().to_utf8();
            match tunnel::resolve_campus(shared, &host, Some(&tuple.destination.ip().to_string()))
                .await
            {
                Ok(resolution) => {
                    let ttl = resolution
                        .expires
                        .map(|deadline| {
                            deadline
                                .saturating_duration_since(std::time::Instant::now())
                                .as_secs()
                                .min(u64::from(u32::MAX)) as u32
                        })
                        .unwrap_or(0);
                    for address in resolution.addresses {
                        let data = match (question.query_type(), address) {
                            (RecordType::A, std::net::IpAddr::V4(address)) => {
                                Some(RData::A(A(address)))
                            }
                            (RecordType::AAAA, std::net::IpAddr::V6(address)) => {
                                Some(RData::AAAA(AAAA(address)))
                            }
                            _ => None,
                        };
                        if let Some(data) = data {
                            response.add_answer(Record::from_rdata(
                                question.name().clone(),
                                ttl,
                                data,
                            ));
                        }
                    }
                }
                Err(_) => {
                    response.set_response_code(ResponseCode::ServFail);
                }
            }
        }
        let bytes = response
            .to_vec()
            .map_err(|_| VpnFailure::new("dns", "TUN DNS 回答编码失败"))?;
        let header = if tuple.source.is_ipv4() { 28 } else { 48 };
        let bytes = if bytes.len() + header > usize::from(shared.plan.mtu) {
            let mut truncated = Message::new();
            truncated
                .set_id(request.id())
                .set_message_type(MessageType::Response)
                .set_truncated(true)
                .set_recursion_desired(request.recursion_desired())
                .set_recursion_available(true)
                .add_query(question.clone());
            truncated
                .to_vec()
                .map_err(|_| VpnFailure::new("dns", "截断 DNS 回答编码失败"))?
        } else {
            bytes
        };
        let reply = tuple.reverse().packet(&bytes)?;
        shared
            .stats
            .upload
            .fetch_add(packet.len() as u64, Ordering::Relaxed);
        shared
            .stats
            .download
            .fetch_add(reply.len() as u64, Ordering::Relaxed);
        outgoing
            .send(reply)
            .await
            .map_err(|_| VpnFailure::new("closed", "TUN DNS 端口已关闭"))?;
    }
    Ok(())
}
