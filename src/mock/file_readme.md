# Ledger Lite

A small library that keeps a ledger and splits it into pages.

## Install

```sh
pip install ledger-lite
```

## Use

`paginate(rows, size)` returns the pages of a ledger, `size` rows each. The last page may be shorter.

| Function | Returns |
|---|---|
| `paginate(rows, size)` | a list of pages |
| `last_page(total)` | the index of the last page |

See [the pagination notes](docs/pagination.md) for the details.
