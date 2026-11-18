# Paging

The list endpoint returns **one page** of rows at a time.

## Options

- `page` is the page to read, counting from 1.
- `size` is how many rows a page holds (default 25, at most 100).

## Notes

Keep results ordered by name, so a page never repeats a row.

```
GET /items?page=2&size=10
```
