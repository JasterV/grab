<a id="Author"></a>
## Author

### Definition

```protobuf
package library.domain;

message Author {
  string  id = 1;
  string  full_name = 2;
  repeated library.domain.Book  bibliography = 3;
}
```

### Dependencies

- Field `bibliography`: [Book](library.domain.md#Book)

---

<a id="Book"></a>
## Book

### Definition

```protobuf
package library.domain;

message Book {
  string  isbn = 1;
  string  title = 2;
  library.domain.Author  author = 3;
  library.domain.Publisher  publisher = 4;
  library.domain.Genre  genre = 5;
}
```

### Dependencies

- Field `author`: [Author](library.domain.md#Author)
- Field `publisher`: [Publisher](library.domain.md#Publisher)
- Field `genre`: [Genre](library.domain.md#Genre)

---

<a id="Publisher"></a>
## Publisher

### Definition

```protobuf
package library.domain;

message Publisher {
  string  id = 1;
  string  name = 2;
  string  address = 3;
}
```

### Dependencies

*None*

---

<a id="Genre"></a>
## Genre

### Definition

```protobuf
package library.domain;

enum Genre {
  UNKNOWN = 0;
  FICTION = 1;
  NON_FICTION = 2;
  SCI_FI = 3;
  HISTORY = 4;
}
```

---

