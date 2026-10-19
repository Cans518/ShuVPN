// Independent fixture encoder for the fixed V2Fly routercommon .proto schema.
pub fn varint(mut value: u64) -> Vec<u8> {
    let mut bytes = vec![];
    while value >= 128 {
        bytes.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    bytes.push(value as u8);
    bytes
}

pub fn number(field: u32, value: u64) -> Vec<u8> {
    [varint(u64::from(field) << 3), varint(value)].concat()
}

pub fn bytes(field: u32, value: &[u8]) -> Vec<u8> {
    [
        varint((u64::from(field) << 3) | 2),
        varint(value.len() as u64),
        value.to_vec(),
    ]
    .concat()
}

pub fn site_domain(kind: u64, value: &str, attributes: &[(&str, u64)]) -> Vec<u8> {
    let mut message = [number(1, kind), bytes(2, value.as_bytes())].concat();
    for (key, value) in attributes {
        let attribute = [bytes(1, key.as_bytes()), number(2, *value)].concat();
        message.extend(bytes(3, &attribute));
    }
    bytes(2, &message)
}

pub fn site() -> Vec<u8> {
    let group = [
        bytes(1, b"TeSt"),
        site_domain(0, "track", &[("ads", 0)]),
        site_domain(1, "^api[0-9]{1,3}[.]example$", &[("ads", 1)]),
        site_domain(2, "campus.example", &[("ads", 0), ("CN", 1)]),
        site_domain(3, "exact.example", &[]),
    ]
    .concat();
    bytes(1, &group)
}

pub fn ip(inverse: bool) -> Vec<u8> {
    let ipv4 = [bytes(1, &[10, 0, 0, 0]), number(2, 8)].concat();
    let ipv6 = [
        bytes(
            1,
            &[0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        number(2, 32),
    ]
    .concat();
    bytes(
        1,
        &[
            bytes(1, b"CN"),
            bytes(2, &ipv4),
            bytes(2, &ipv6),
            number(3, u64::from(inverse)),
        ]
        .concat(),
    )
}
