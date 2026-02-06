<a id="ChatMessage"></a>
## ChatMessage

### Definition

```protobuf
package library.rpc;

message ChatMessage {
  string  user_id = 1;
  string  text = 2;
  int64  timestamp = 3;
}
```

### Dependencies

*None*

---

<a id="CheckoutRequest"></a>
## CheckoutRequest

### Definition

```protobuf
package library.rpc;

message CheckoutRequest {
  string  isbn = 1;
}
```

### Dependencies

*None*

---

<a id="CheckoutResponse"></a>
## CheckoutResponse

### Definition

```protobuf
package library.rpc;

message CheckoutResponse {
  repeated library.domain.Book  checked_out_books = 1;
  int32  total_items = 2;
  string  due_date = 3;
}
```

### Dependencies

- Field `checked_out_books`: [Book](library.domain.md#Book)

---

<a id="GetBookRequest"></a>
## GetBookRequest

### Definition

```protobuf
package library.rpc;

message GetBookRequest {
  string  isbn = 1;
}
```

### Dependencies

*None*

---

<a id="QueryBooksRequest"></a>
## QueryBooksRequest

### Definition

```protobuf
package library.rpc;

message QueryBooksRequest {
  string  title_prefix = 1;
  library.domain.Genre  genre_filter = 2;
}
```

### Dependencies

- Field `genre_filter`: [Genre](library.domain.md#Genre)

---

