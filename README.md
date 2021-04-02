# ETM - Easy TCP Messaging
This crate is used fot the TCP messaging. Clients which don't use this crate must implement the following protocol

## Transport Data Types

+ defined in transport.rs
+ enum with transport __Types__ and __Transmission__ struct

### Transport Types

```
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum Type<T> {
    Error(T),
    End,
    Request(T),
    Response(T),
    Stream(T),
}
```

This are the type definitions for the transmission payload
+ `Error(T)`: error of type T; currently a string
+ `End`: signals the end of a communication, e.g. a Stream
+ `Request(T)`: user defined request of type T
+ `Response(T)`: user defined response of type T for a previous request
+ `Stream(T)`: user defined stream of type T

### Transmission struct

```
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Transmission<T> {
    pub id: u64,
    pub r#type: Type<T>,
}
```
+ `id`: consecutive number; id of a Response type must be equal to the corresponding Request id
    + TODO: should there be an ANY(e.g. 0) and an INVALID(e.g. -1u) id?
+ `type`: the transport Type with data of type T

## Management Data Types

+ defined in mgmt.rs
+ enum with __Request__ and __Response__ types and data structs

### Request and Response Types

```
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum Request {
    Identify{protocol_version: u32},
    Connect(CommParams),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum Response {
    Identify(Identity),
    Connect(CommSettings),
}
```

### Identification Request and Response

Before a communication starts, the client should do an Identification request to check if the server delivers the expected service. If the server doesn't deliver the expected service and the communication is nevertheless established, the land of undefined behaviour is entered, populated with unicorns and pink elephants.

#### Request
```
protocol_version: u32
```
+ `protocol_version`: the etm protocol version used by the client as defined in lib.rs

#### Response
```
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Identity {
    pub protocol_version: u32,
    pub service: Service,
}
```

+ `protocol_version`: the etm protocol version used by the server as defined in lib.rs; client and server protocol versions should be equal; a cute little pony dies if the communication proceeds with dissimilar versions
+ `service`: a description of the service the server provides; client and server service descripions should be equal; a cute little pony dies if the communication proceeds with dissimilar service descripions


### Connection Request and Response

#### Request
```
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct CommParams {
    pub protocol_version: u32,
    pub connection_id: u32,           // -1 dynamic
    pub rpc_interval_timeout_ms: u32, // -1 infinite
}
```

+ `protocol_version`: the etm protocol version used by the client as defined in lib.rs
+ `connection_id`: placeholder; use -1; once implemented, the server will have only one connection for a connection_id; if there is already an open connection, this will be closed before a new connection with the same id is opened; -1 will assign an unused id
+ `rpc_interval_timeout_ms`: placeholder; use -1; once implemented the client has to send RPCs within the defined interval else the server closes the connection; a value of -1 indicates an infinite timeout


#### Response
```
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct CommSettings {
    pub connection_id: u32, // assigned connection id
    pub port: u16,
}
```

+ `connection_id`: placeholder
+ `port`: the assigned tcp port for the RPCs; the port has to be opened within 2 seconds else stops listening on that port

## Service description

+ defined in lib.rs

```
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Service {
    id: String,
    protocol_version: u32,
}
```

+ `id`: name of the service
+ `protocol_version`: the protocol version of the service

## Establish the connection and transmit RPCs
```
Client                                                         Server
  |                                                              |
  |  client opens the management port (e.g. 0xABBA)              |
  |    and transmits an Identify Request                         |
  | -----------------------------------------------------------> |
  |                                                              |
  |                       server transmits an Identify Response  |
  | <----------------------------------------------------------- |
  |                                                              |
  |  client closes the management port (e.g. 0xABBA),            |
  | -----------------------------------------------------------> |
  |                                                              |
  |  client checks if the server delivers the expected service,  |
  |    opens the management port (e.g. 0xABBA)                   |
  |    and transmits a Connect Request                           |
  | -----------------------------------------------------------> |
  |                                                              |
  |                         server transmits a Connect Response  |
  | <----------------------------------------------------------- |
  |                                                              |
  |  client opens port from ConnectSettings                      |
  | -----------------------------------------------------------> |
  |                                                              |
  |  client closes the management port (e.g. 0xABBA)             |
  | -----------------------------------------------------------> |
  |                                                              |
  |                                                              |
  |  client transmits a rpc Request                              |
  | -----------------------------------------------------------> |
  |                                                              |
  |                             server transmits a rpc Response  |
  | <----------------------------------------------------------- |
  |                            .                                 |
  |                            .                                 |
  |                            .                                 |
  |                                                              |
  |  client transmits a rpc Request                              |
  | -----------------------------------------------------------> |
  |                                                              |
  |                             server transmits a rpc Response  |
  | <----------------------------------------------------------- |
  |                                                              |
  |  client closes port                                          |
  | -----------------------------------------------------------> |
  |                                                              |
```

## Transmissions
A transmission consists of 8 bytes length of the serialized __Transmission__ followed by the serialized __Transmission__ itself. Everything is encoded in network order.

### Example 1: Custom RPC Request and Response
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
Therefore we have `Transmission<MyRequest>` as type.

Ping/Pong RPC
```
                                         Header                                   Payload
        +-----------------------------------------------------------------+------------------------+
        |         length of                                     transport |   enum tag             |
        |     remaining message          transmission id          type    | MyRequest[Ping]        |
        | ___________/\__________   ___________/\__________   _____/\____ | _____/\____            |
        |/                       \ /                       \ /           \|/           \   no enum |
Client: |0x00 00 00 00 00 00 00 10 0x00 00 00 00 00 00 00 2A 0x00 00 00 02|0x00 00 00 00   content |
        |                                                                 |               therefore|
Server: |0x00 00 00 00 00 00 00 10 0x00 00 00 00 00 00 00 2A 0x00 00 00 03|0x00 00 00 00   no data |
        |\___________  __________/ \___________  __________/ \_____  ____/|\_____  ____/           |
        |            \/                        \/                  \/     |      \/                |
        |        length of               transmission id        transport |   enum tag             |
        |     remaining message                                   type    | MyResponse[Pong]       |
        +-----------------------------------------------------------------+------------------------+
```




Answer RPC
```                                                                           enum tag
                  length of                                     transport    MyRequest
              remaining message          transmission id          type        [Answer]     no enum
          ___________/\__________   ___________/\__________   _____/\____   _____/\____    content
         /                       \ /                       \ /           \ /           \  therefore
Client:  0x00 00 00 00 00 00 00 10 0x00 00 00 00 00 00 00 2B 0x00 00 00 02 0x00 00 00 01   no data

Server:  0x00 00 00 00 00 00 00 14 0x00 00 00 00 00 00 00 2B 0x00 00 00 03 0x00 00 00 01 0x00 00 00 2A
         \___________  __________/ \___________  __________/ \_____  ____/ \_____  ____/ \_____  ____/
                     \/                        \/                  \/            \/            \/
                 length of               transmission id        transport     enum tag    enum content
              remaining message                                   type       MyResponse       [42]
                                                                              [Answer]
```


### Example2: Management Identify request and response
```
                  length of                                     transport   management    etm protocol
              remaining message          transmission id          type     request type      version
          ___________/\__________   ___________/\__________   _____/\____   _____/\____   _____/\____
         /                       \ /                       \ /           \ /           \ /           \
Client:  0x00 00 00 00 00 00 00 14 0x00 00 00 00 00 00 00 0D 0x00 00 00 02 0x00 00 00 00 0x00 00 00 00

Server:  0x00 00 00 00 00 00 00 29 0x00 00 00 00 00 00 00 0D 0x00 00 00 03 0x00 00 00 00 ...
         \___________  __________/ \___________  __________/ \_____  ____/ \_____  ____/
                     \/                        \/                  \/            \/
                 length of               transmission id        transport    management
              remaining message                                   type     response type

         ...  0x00 00 00 00 0x00 00 00 00 00 00 00 09 0x4D 79 53 65 72 76 69 63 65 0x00 00 00 05
              \_____  ____/ \________________________| __________________________/ \_____  ____/
                    \/                               \/                                  \/
               etm protocol    id (string length and data "MyService")               "MyService"
                  version                                                         protocol version
```
