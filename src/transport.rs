use serde::{Deserialize, Serialize};

pub type Error = String;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum Type<T> {
    Error(T),
    End,
    Request(T),
    Response(T),
    Stream(T),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Transmission<T> {
    pub id: u64, // maybe tag instead of id?
    pub r#type: Type<T>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use bincode::Options;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    enum TestType {
        U8(u8),
        String(String),
        Vec(Vec<u8>),
    }

    #[test]
    fn error_transmission() {
        let serde = bincode::DefaultOptions::new()
            .with_big_endian()
            .with_fixint_encoding();

        let transmission = Transmission::<TestType> {
            id: 0x42,
            r#type: Type::Error(TestType::U8(0x13)),
        };

        let transmission = serde.serialize(&transmission);
        assert!(transmission.is_ok());

        // 8 byte transmission id, 4 byte transmission type tag, 4 byte test type tag, 1 byte test type value
        const EXPECTED: [u8; 17] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x13,
        ];

        if let Ok(result) = transmission {
            assert_eq!(result, EXPECTED);
        }
    }

    #[test]
    fn end_transmission() {
        let serde = bincode::DefaultOptions::new()
            .with_big_endian()
            .with_fixint_encoding();

        let transmission = Transmission::<TestType> {
            id: 0x42,
            r#type: Type::End,
        };

        let transmission = serde.serialize(&transmission);
        assert!(transmission.is_ok());

        // 8 byte transmission id, 4 byte transmission type tag, 4 byte test type tag
        const EXPECTED: [u8; 12] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42, 0x00, 0x00, 0x00, 0x01,
        ];

        if let Ok(result) = transmission {
            assert_eq!(result, EXPECTED);
        }
    }

    #[test]
    fn request_transmission() {
        let serde = bincode::DefaultOptions::new()
            .with_big_endian()
            .with_fixint_encoding();

        let transmission = Transmission::<TestType> {
            id: 0x42,
            r#type: Type::Request(TestType::U8(0x13)),
        };

        let transmission = serde.serialize(&transmission);
        assert!(transmission.is_ok());

        // 8 byte transmission id, 4 byte transmission type tag, 4 byte test type tag, 1 byte test type value
        const EXPECTED: [u8; 17] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00,
            0x00, 0x00, 0x13,
        ];

        if let Ok(result) = transmission {
            assert_eq!(result, EXPECTED);
        }
    }

    #[test]
    fn response_transmission() {
        let serde = bincode::DefaultOptions::new()
            .with_big_endian()
            .with_fixint_encoding();

        let transmission = Transmission::<TestType> {
            id: 0x42,
            r#type: Type::Response(TestType::U8(0x13)),
        };

        let transmission = serde.serialize(&transmission);
        assert!(transmission.is_ok());

        // 8 byte transmission id, 4 byte transmission type tag, 4 byte test type tag, 1 byte test type value
        const EXPECTED: [u8; 17] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00,
            0x00, 0x00, 0x13,
        ];

        if let Ok(result) = transmission {
            assert_eq!(result, EXPECTED);
        }
    }

    #[test]
    fn stream_transmission() {
        let serde = bincode::DefaultOptions::new()
            .with_big_endian()
            .with_fixint_encoding();

        let transmission = Transmission::<TestType> {
            id: 0x42,
            r#type: Type::Stream(TestType::U8(0x13)),
        };

        let transmission = serde.serialize(&transmission);
        assert!(transmission.is_ok());

        // 8 byte transmission id, 4 byte transmission type tag, 4 byte test type tag, 1 byte test type value
        const EXPECTED: [u8; 17] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00,
            0x00, 0x00, 0x13,
        ];

        if let Ok(result) = transmission {
            assert_eq!(result, EXPECTED);
        }
    }

    #[test]
    fn transmission_type_string() {
        let serde = bincode::DefaultOptions::new()
            .with_big_endian()
            .with_fixint_encoding();

        let transmission = Transmission::<TestType> {
            id: 0x42,
            r#type: Type::Request(TestType::String("A".to_string())),
        };

        let transmission = serde.serialize(&transmission);
        assert!(transmission.is_ok());

        // 8 byte transmission id, 4 byte transmission type tag, 4 byte test type tag, 9 byte test type value (8 bytes string length, 1 byte string)
        const EXPECTED: [u8; 25] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x41,
        ];

        if let Ok(result) = transmission {
            assert_eq!(result, EXPECTED);
        }
    }

    #[test]
    fn transmission_type_vec() {
        let serde = bincode::DefaultOptions::new()
            .with_big_endian()
            .with_fixint_encoding();

        let transmission = Transmission::<TestType> {
            id: 0x42,
            r#type: Type::Request(TestType::Vec(vec![0x37, 0x73])),
        };

        let transmission = serde.serialize(&transmission);
        assert!(transmission.is_ok());

        // 8 byte transmission id, 4 byte transmission type tag, 4 byte test type tag, 9 byte test type value (8 bytes string length, 1 byte string)
        const EXPECTED: [u8; 26] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00,
            0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x37, 0x73,
        ];

        if let Ok(result) = transmission {
            assert_eq!(result, EXPECTED);
        }
    }
}
