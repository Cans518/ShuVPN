use super::failure;
use crate::routing::model::RoutingFailure;

pub(super) enum Value<'a> {
    Number(u64),
    Bytes(&'a [u8]),
    Fixed,
}

pub(super) struct Message<'a> {
    bytes: &'a [u8],
    position: usize,
    fields: usize,
}

impl<'a> Message<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            position: 0,
            fields: 0,
        }
    }

    pub(super) fn next(&mut self) -> Result<Option<(u32, Value<'a>)>, RoutingFailure> {
        if self.position == self.bytes.len() {
            return Ok(None);
        }
        self.fields += 1;
        if self.fields > 2_000_000 {
            return Err(failure("protobuf 字段数量超过解析预算"));
        }
        let tag = self.number()?;
        let field = u32::try_from(tag >> 3).map_err(|_| failure("protobuf 字段编号溢出"))?;
        if field == 0 || field > 0x1fff_ffff {
            return Err(failure("protobuf 字段编号无效"));
        }
        let value = match tag & 7 {
            0 => Value::Number(self.number()?),
            1 => {
                self.take(8)?;
                Value::Fixed
            }
            2 => {
                let length =
                    usize::try_from(self.number()?).map_err(|_| failure("protobuf 长度溢出"))?;
                Value::Bytes(self.take(length)?)
            }
            5 => {
                self.take(4)?;
                Value::Fixed
            }
            _ => return Err(failure("protobuf wire 类型不支持")),
        };
        Ok(Some((field, value)))
    }

    fn number(&mut self) -> Result<u64, RoutingFailure> {
        let mut value = 0;
        for shift in (0..70).step_by(7) {
            let byte = self.take(1)?[0];
            if shift == 63 && byte > 1 {
                return Err(failure("protobuf varint 溢出"));
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(failure("protobuf varint 未结束"))
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], RoutingFailure> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| failure("protobuf 长度溢出"))?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| failure("protobuf 消息截断"))?;
        self.position = end;
        Ok(bytes)
    }
}

pub(super) fn text(bytes: &[u8]) -> Result<&str, RoutingFailure> {
    if bytes.len() > 4096 {
        return Err(failure("GeoData 字符串超过预算"));
    }
    std::str::from_utf8(bytes).map_err(|_| failure("GeoData 字符串不是 UTF-8"))
}
