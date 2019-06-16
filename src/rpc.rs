/*
 * Copyright (C) 2019 Mathias Kraus <k.hias@gmx.de> - All Rights Reserved
 *
 * Unauthorized copying of this file, via any medium is strictly prohibited
 * Proprietary and confidential
 */

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct RPCRequest<T> {
    pub transmission_id: u32,
    pub data: T,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct RPCResponse<T, E> {
    pub transmission_id: u32,
    pub data: Result<T, E>,
}

pub type ETMError = String;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
