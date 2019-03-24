# ETM - Easy TCP Messaging
This crate is used fot the TCP messaging. Clients which don't use this crate must implement the following protocol

## Data Types

### struct to request a new connection from the server

```
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct ConnectionRequest {
    pub protocol_version: u32,
    pub connection_id: u32,             // -1 dynamic
    pub rpc_interval_timeout_ms: u32,   // -1 infinite
}
```

+ `protocol_version`: the etm protocol version used by the client as defined in lib.rs
+ `connection_id`: placeholder; use -1; once implemented, the server will have only one connection for a connection_id; if there is already an open connection, this will be closed before a new connection with the same id is opened; -1 will assign an unused id
+ `rpc_interval_timeout_ms`: placeholder; use -1; once implemented the client has to send RPCs within the defined interval else the server closes the connection; a value of -1 indicates an infinite timeout


### struct for response to a connection request with the information to establish the new connection

```
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct ConnectionResponse {
    pub protocol_version: u32,
    pub port: u16,
    pub connection_id: u32,         // assigned connection id
    pub service: Service,
}
```

+ `protocol_version`: the etm protocol version used by the server as defined in lib.rs; client and server protocol versions should be equal; a cute little pony dies if the communication proceeds with dissimilar versions
+ `port`: the assigned tcp port for the RPCs; the port has to be opened within 2 seconds else stops listening on that port
+ `connection_id`: placeholder
+ `service`: a description of the service the server provides; client and server service descripions should be equal; a cute little pony dies if the communication proceeds with dissimilar service descripions


### struct for service description

```
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Service {
    id: String,
    protocol_version: u32,
}
```

+ `id`: name of the service
+ `protocol_version`: the protocol version of the service


### struct for a RPC request of type T

```
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct RPCRequest<T> {
    pub transmission_id: u32,
    pub data: T,
}
```

+ `transmission_id`: consecutive number; request and response id must be equal
+ `data`: of type T


### struct for resonse of a RPC request with a Result of type T or error E

```
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct RPCResponse<T, E> {
    pub transmission_id: u32,
    pub data: Result<T, E>,
}
```

+ `transmission_id`: consecutive number; request and response id must be equal
+ `data`: Result of type T or error E


## Establish the connection and transmit RPCs
```
Client                                                         Server
  |                                                              |
  |  client opens connection reqest port (e.g. 0xABBA)           |
  |    and transmits a ConnectionRequest                         |
  | -----------------------------------------------------------> |
  |                                                              |
  |                       server transmits a ConnectionResponse  |
  | <----------------------------------------------------------- |
  |                                                              |
  |  client opens port from ConnectionResponse                   |
  | -----------------------------------------------------------> |
  |                                                              |
  |  client closes connection request port (e.g. 0xABBA)         |
  | -----------------------------------------------------------> |
  |                                                              |
  |                                                              |
  |  client transmits a RPCRequest                               |
  | -----------------------------------------------------------> |
  |                                                              |
  |                              server transmits a RPCResponse  |
  | <----------------------------------------------------------- |
  |                            .                                 |
  |                            .                                 |
  |                            .                                 |
  |                                                              |
  |  client transmits a RPCRequest                               |
  | -----------------------------------------------------------> |
  |                                                              |
  |                              server transmits a RPCResponse  |
  | <----------------------------------------------------------- |
  |                                                              |
  |  client closes port                                          |
  | -----------------------------------------------------------> |
  |                                                              |
```

## Transmissions
A transmission consists of 8 bytes length of the serialized data followed by the serialized ConnectionRequest or ConnectionResponse or RPCRequest or RPCResponse. Everything is encoded in networg order.

Example: ConnectionRequest and ConnectionResponse
```
                  length of         etm protocol   connection    rpc interval
              remaining message        version         id          timeout
          ___________/\__________   _____/\____   _____/\____   _____/\____
         /                       \ /           \ /           \ /           \
Client:  0x00 00 00 00 00 00 00 0C 0x00 00 00 00 0xFF FF FF FF 0xFF FF FF FF

Server:  0x00 00 00 00 00 00 00 1F 0x00 00 00 00 0xFF FF FF FF 0xA2 D2...
         \___________  __________/ \_____  ____/ \_____  ____/ \__  _/
                     \/                  \/            \/         \/
                 length of          etm protocol   connection    port
              remaining message        version         id

         ...  0x00 00 00 00 00 00 00 09 0x4D 79 53 65 72 76 69 63 65 0x00 00 00 00
              \________________________| __________________________/ \_____  ____/
                                       \/                                  \/
                 id (string length and data "MyService")            protocol version
```


Example: RPC Request and Response
Let's assume we use the following types for the RPCs.
```
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum MyRequest {
    Ping(),
    Answer(),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum MyResponse {
    Pong(),
    Answer(u32),
}

pub type MyError = String;
```
Therefore we have `RPCRequest<MyRequest>` and `RPCResponse<MyResponse, MyError>` as types.

Ping/Pong RPC
```
                  length of         transmission    enum tag
              remaining message          id      MyRequest[Ping]
          ___________/\__________   _____/\____   _____/\____
         /                       \ /           \ /           \   no enum content
Client:  0x00 00 00 00 00 00 00 08 0x00 00 00 2A 0x00 00 00 00  therefore no data

Server:  0x00 00 00 00 00 00 00 0C 0x00 00 00 2A 0x00 00 00 00 0x00 00 00 00   no enum content
         \___________  __________/ \_____  ____/ \_____  ____/ \_____  ____/  therefore no data
                     \/                  \/            \/            \/
                 length of          transmission    enum tag      enum tag
              remaining message          id         Result[T]  MyResponse[Pong]
```

Answer RPC
```
                  length of         transmission    enum tag
              remaining message          id     MyRequest[Answer]
          ___________/\__________   _____/\____   _____/\____
         /                       \ /           \ /           \   no enum content
Client:  0x00 00 00 00 00 00 00 08 0x00 00 00 2A 0x00 00 00 01  therefore no data

Server:  0x00 00 00 00 00 00 00 10 0x00 00 00 2A 0x00 00 00 00 0x00 00 00 01 0x00 00 00 2A
         \___________  __________/ \_____  ____/ \_____  ____/ \_____  ____/ \_____  ____/
                     \/                  \/            \/            \/            \/
                 length of          transmission    enum tag      enum tag    enum content
              remaining message          id         Result[T]    MyResponse       [42]
                                                                  [Answer]
```
