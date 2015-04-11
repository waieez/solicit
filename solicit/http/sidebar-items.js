initSidebarItems({"mod":[["connection","The module contains the implementation of an HTTP/2 connection."],["frame","The module contains the implementation of HTTP/2 frames."],["session","Defines the interface for the session-level management of HTTP/2 communication. This is effectively an API that allows hooking into an HTTP/2 connection in order to handle events arising on the connection."],["transport","The module contains implementations of the transport layer functionality that HTTP/2 requires. It exposes APIs that allow the HTTP/2 connection to use the transport layer without requiring it to know which exact implementation they are using (i.e. a clear-text TCP connection, a TLS protected connection, or even a mock implementation)."]],"struct":[["Request","A struct representing a full HTTP/2 request, along with the full body, as a sequence of bytes."],["Response","A struct representing the full raw response received on an HTTP/2 connection."]],"type":[["Header","An alias for the type that represents HTTP/2 haders. For now we only alias the tuple of byte vectors instead of going with a full struct representation."],["HttpResult","A convenience `Result` type that has the `HttpError` type as the error type and a generic Ok result type."],["StreamId","An alias for the type that represents the ID of an HTTP/2 stream"]],"enum":[["HttpError","An enum representing errors that can arise when performing operations involving an HTTP/2 connection."],["HttpScheme","An enum representing the two possible HTTP schemes."]],"constant":[["ALPN_PROTOCOLS","A set of protocol names that the library should use to indicate that HTTP/2 is supported during protocol negotiation (NPN or ALPN). We include some of the drafts' protocol names, since there is basically no difference for all intents and purposes (and some servers out there still only officially advertise draft support). TODO: Eventually only use \"h2\"."]]});