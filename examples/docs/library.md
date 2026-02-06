<a id="LibraryService"></a>
## LibraryService

### Definition

```protobuf
package library;

service LibraryService {
  rpc GetBook(library.rpc.GetBookRequest) returns (library.domain.Book);

  rpc QueryBooks(library.rpc.QueryBooksRequest) returns (stream library.domain.Book);

  rpc Checkout(stream library.rpc.CheckoutRequest) returns (library.rpc.CheckoutResponse);

  rpc SupportChat(stream library.rpc.ChatMessage) returns (stream library.rpc.ChatMessage);

}
```

### Methods

#### `GetBook`

- Request: [GetBookRequest](library.rpc.md#GetBookRequest)
- Response: [Book](library.domain.md#Book)

#### `QueryBooks`

- Request: [QueryBooksRequest](library.rpc.md#QueryBooksRequest)
- Response: [Book](library.domain.md#Book)

#### `Checkout`

- Request: [CheckoutRequest](library.rpc.md#CheckoutRequest)
- Response: [CheckoutResponse](library.rpc.md#CheckoutResponse)

#### `SupportChat`

- Request: [ChatMessage](library.rpc.md#ChatMessage)
- Response: [ChatMessage](library.rpc.md#ChatMessage)

---

