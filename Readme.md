# Overview
This project is for exploring networking, distributed systems, and IPC.

I'd like to extract at least some of the following as reusable crates, if I can find reasonable abstraction points and API:
* Framing and buffering for mio
* Simple library to handle frames on the network
* Simple library to serialize/deserializes to/from frames

# TODO
* Tests
* Profiling
* Finish context refactoring
* Stop screwing around with performance before I have profiling and benchmarks
* More context APIs
  * unix_listen
  * tcp_listen
  * unix_connect
  * tcp_connect
  * close
  * ssl?
  * ssh?
    * No universal way to connect to unix socket over ssh
    * Need to rely on helped programs
      * I couldn't get `nc -U socketpath` to work
      * `socat - UNIX-CONNECT:socketpath` works well, but socat isn't very commonly present
      * Maybe best to include small helper program
* Parameterize frame length type (u16/u32/u64)?