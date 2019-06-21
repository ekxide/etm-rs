/*
 * Copyright (C) 2019 Mathias Kraus <k.hias@gmx.de> - All Rights Reserved
 *
 * Unauthorized copying of this file, via any medium is strictly prohibited
 * Proprietary and confidential
 */

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
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
