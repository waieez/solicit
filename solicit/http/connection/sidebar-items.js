initSidebarItems({"enum":[["HttpFrame","An enum representing all frame variants that can be returned by an `HttpConnection`."],["TlsConnectError","An enum representing possible errors that can arise when trying to establish an HTTP/2 connection over TLS."]],"trait":[["HttpConnect","A trait that can be implemented by structs that want to provide the functionality of establishing HTTP/2 connections."],["HttpConnectError","A marker trait for errors raised by attempting to establish an HTTP/2 connection."]],"struct":[["CleartextConnectError","A newtype wrapping the `io::Error`, as it occurs when attempting to establish an HTTP/2 connection over cleartext TCP (with prior knowledge)."],["CleartextConnector","A struct that establishes an HTTP/2 connection based on a prior-knowledge cleartext TCP connection. It defaults to using port 80 on the given host."],["ClientConnection","A struct implementing the client side of an HTTP/2 connection."],["HttpConnection","The struct implements the HTTP/2 connection level logic."],["TlsConnector","A struct implementing the functionality of establishing an HTTP/2 connection over a TLS-backed stream. The protocol negotiation also takes place within the TLS negotiation."]]});