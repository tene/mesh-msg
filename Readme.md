# Overview
This project is for exploring networking, distributed systems, and IPC.

I'd like to extract at least some of the following as reusable crates, if I can find reasonable abstraction points and API:
* Framing and buffering for mio
* Simple library to handle frames on the network
* Simple library to serialize/deserializes to/from frames

# Todo

* Don't bother with generalizing mio-framed-serde, just implement it in the app; you can abstract later
* Agent/Server protocol distinction?
* Hello
* Ping
* Relay direct
* Exchange routes
  * Nodemap?
* Service abstraction?
* Publish/Subscribe?
* Filesystem service?
* Command execution?
* Process Tree?