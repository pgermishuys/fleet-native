#!/usr/bin/env python3
"""A stand-in for GitHub's routes in front of a scratch Fleet (component 21).

It answers GET|POST|PUT|DELETE /api/integrations/github/* and GET /api/smart-links from a JSON fixture of invented data
(scripts/github-standin.json), keeping bookmarks in memory, and passes every other request through untouched: a WebSocket or an
event stream included (after an Upgrade the two sockets are spliced). Point both clients at it, the native app with
`--server http://127.0.0.1:<port>` and Fleet's own client in a browser at the same address, and both see the same GitHub.

    python3 scripts/github-standin.py --listen 6431 --fleet 6411 [--fixture FILE] [--link <session-id>=<owner/repo#n> ...] [--repos-delay SECONDS] [--repos-fail]

Python standard library only. Never point it at a real Fleet: it is for a scratch one, and it holds no token.
"""
import argparse
import json
import re
import socket
import threading
import time
from urllib.parse import parse_qs, urlsplit

GH = "/api/integrations/github/"


def read_head(conn):
    data = b""
    while b"\r\n\r\n" not in data:
        chunk = conn.recv(65536)
        if not chunk:
            return None, b""
        data += chunk
    head, _, rest = data.partition(b"\r\n\r\n")
    return head.decode("latin-1"), rest


def respond(conn, status, body):
    raw = b"" if body is None else json.dumps(body).encode()
    reason = {200: "OK", 204: "No Content", 400: "Bad Request", 404: "Not Found"}.get(status, "OK")
    conn.sendall(
        f"HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {len(raw)}\r\nConnection: close\r\n\r\n".encode() + raw
    )


class Standin:
    def __init__(self, fixture, fleet_port, links, repos_delay=0.0, repos_fail=False):
        self.repos_delay = repos_delay
        self.repos_fail = repos_fail
        self.fx = fixture
        self.fleet = fleet_port
        self.links = links
        self.lock = threading.Lock()

    def answer(self, method, target, body):
        url = urlsplit(target)
        path, query = url.path, parse_qs(url.query)
        fx = self.fx
        if path == "/api/smart-links" and method == "GET":
            return 200, self.links
        if not path.startswith(GH):
            return None
        route = path[len(GH):].strip("/")
        if route == "auth/status":
            return 200, {"connected": fx["connected"]}
        if route == "auth" and method == "DELETE":
            fx["connected"] = False
            return 204, None
        if route == "auth/token" and method == "POST":
            fx["connected"] = True
            return 200, {}
        if not fx["connected"]:
            return 401, {"error": "GitHub is not connected."}
        if route == "work":
            return 200, fx["work"]
        if route == "repos":
            time.sleep(self.repos_delay)
            if self.repos_fail:
                return 403, {"error": "GitHub rate limit exceeded. Try again in a few minutes."}
            page, per = int(query.get("page", ["1"])[0]), int(query.get("per_page", ["30"])[0])
            return 200, fx["repos"][(page - 1) * per : page * per]
        if route == "bookmarks":
            with self.lock:
                if method == "GET":
                    return 200, fx["bookmarks"]
                if method == "PUT":
                    fx["bookmarks"] = (json.loads(body or b"{}") or {}).get("bookmarks", [])
                    return 200, fx["bookmarks"]
                if method == "POST":
                    name = (json.loads(body or b"{}") or {}).get("repo", "")
                    owner, _, repo = name.partition("/")
                    if not owner or not repo:
                        return 400, {"error": "Invalid repository name."}
                    if not any(b["fullName"] == name for b in fx["bookmarks"]):
                        fx["bookmarks"].append({"fullName": name, "owner": owner, "name": repo})
                    return 200, fx["bookmarks"]
        if route == "search":
            return 200, self.search(query.get("q", [""])[0], int(query.get("first", ["30"])[0]), query.get("after", [""])[0])
        m = re.fullmatch(r"repos/([^/]+)/([^/]+)/items", route)
        if m and "numbers" in query:
            want = [int(n) for n in query["numbers"][0].split(",") if n.isdigit()]
            return 200, [i for i in fx["items"] if i["owner"] == m[1] and i["repo"] == m[2] and i["number"] in want]
        m = re.fullmatch(r"repos/([^/]+)/([^/]+)/(labels|milestones|assignees)", route)
        if m:
            return 200, fx.get(m[3], {}).get(f"{m[1]}/{m[2]}", [])
        m = re.fullmatch(r"bookmarks/([^/]+)/([^/]+)", route)
        if m and method == "DELETE":
            with self.lock:
                fx["bookmarks"] = [b for b in fx["bookmarks"] if b["fullName"] != f"{m[1]}/{m[2]}"]
            return 204, None
        return 404, {"error": "The stand-in does not know this route."}

    def search(self, q, first, after):
        """The fixture's items that a search's qualifiers keep: repo:, is:pr|issue|open|closed, author:@me, review-requested:@me, label:, milestone:, assignee:, no:assignee,
        sort:{updated,created,comments}-{asc,desc}, and any other word as part of the title. Paged by position (the cursor is the next position)."""
        fx = self.fx
        me = fx["work"]["login"]
        asked = {(i["repo"], i["number"]) for i in fx["work"]["reviewRequested"]}
        words = re.findall(r'(?:[^\s"]+:)?"[^"]*"|\S+', q)
        items = list(fx["items"])
        for w in words:
            key, _, val = w.partition(":")
            val = val.strip('"')
            if key == "repo":
                items = [i for i in items if f"{i['owner']}/{i['repo']}" == val]
            elif w == "is:pr":
                items = [i for i in items if i["kind"] == "pull"]
            elif w == "is:issue":
                items = [i for i in items if i["kind"] == "issue"]
            elif w == "is:open":
                items = [i for i in items if i["state"] == "open"]
            elif w == "is:closed":
                items = [i for i in items if i["state"] != "open"]
            elif key == "author":
                items = [i for i in items if i["author"] == (me if val == "@me" else val)]
            elif key == "review-requested":
                items = [i for i in items if (i["repo"], i["number"]) in asked]
            elif key == "label":
                items = [i for i in items if any(l["name"] == val for l in i["labels"])]
            elif key == "milestone":
                items = [i for i in items if (i.get("milestone") is None) == (val == "none") and (val == "none" or i.get("milestone") == val)]
            elif key == "assignee":
                items = [i for i in items if val in i["assignees"]]
            elif w == "no:assignee":
                items = [i for i in items if not i["assignees"]]
            elif key == "sort":
                field, _, direction = val.rpartition("-")
                name = {"updated": "updatedAt", "created": "createdAt", "comments": "comments"}.get(field, "updatedAt")
                items.sort(key=lambda i: i[name], reverse=direction != "asc")
            elif ":" not in w:
                items = [i for i in items if w.lower() in i["title"].lower()]
        start = int(after) if after.isdigit() else 0
        page = items[start : start + first]
        more = start + first < len(items)
        return {"items": page, "totalCount": len(items), "endCursor": str(start + first) if more else None, "hasNextPage": more}

    def handle(self, conn):
        try:
            head, rest = read_head(conn)
            if head is None:
                return
            line, _, headers = head.partition("\r\n")
            method, target, _ = line.split(" ", 2)
            length = int(next((h.split(":", 1)[1] for h in headers.split("\r\n") if h.lower().startswith("content-length:")), "0"))
            upgrade = "upgrade:" in headers.lower()
            hit = None
            if not upgrade and (target.startswith(GH) or target.startswith("/api/smart-links")):
                body = rest
                while len(body) < length:
                    body += conn.recv(65536)
                hit = self.answer(method, target, body[:length])
            if hit is not None:
                return respond(conn, *hit)
            up = socket.create_connection(("127.0.0.1", self.fleet))
            if not upgrade:
                head = re.sub(r"(?im)^connection:.*$", "", head) + "\r\nConnection: close"
            up.sendall(head.encode("latin-1") + b"\r\n\r\n" + rest)
            splice(conn, up)
        except OSError:
            pass
        finally:
            conn.close()


def splice(a, b):
    def pump(src, dst):
        try:
            while data := src.recv(65536):
                dst.sendall(data)
        except OSError:
            pass
        finally:
            for s in (src, dst):
                try:
                    s.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass

    t = threading.Thread(target=pump, args=(b, a), daemon=True)
    t.start()
    pump(a, b)
    t.join(1)
    b.close()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--listen", type=int, required=True)
    ap.add_argument("--fleet", type=int, required=True)
    ap.add_argument("--fixture", default=__file__.rsplit("/", 1)[0] + "/github-standin.json")
    ap.add_argument("--repos-delay", type=float, default=0.0, help="seconds each repository page takes (to look at Loading)")
    ap.add_argument("--repos-fail", action="store_true", help="answer the repository list with a refusal")
    ap.add_argument("--link", action="append", default=[], help="<session-id>=<owner/repo#n>")
    a = ap.parse_args()
    fixture = json.load(open(a.fixture))
    links = []
    for spec in a.link:
        sid, _, ref = spec.partition("=")
        m = re.fullmatch(r"([^/]+)/([^#]+)#(\d+)", ref)
        if m:
            owner, repo, number = m[1], m[2], int(m[3])
            meta = {"owner": owner, "repo": repo, "number": number, "htmlUrl": f"https://github.com/{owner}/{repo}/pull/{number}"}
            # What the watcher would have read from GitHub for it (a title, an author, a branch, a diff), from the fixture's "linkMeta".
            extra = fixture.get("linkMeta", {}).get(ref, {})
            title = extra.pop("title", f"Pull request {number}")
            meta.update(extra)
            links.append(
                {
                    "id": f"link-{sid}-{number}", "sessionId": sid, "url": meta["htmlUrl"], "providerId": "github", "resourceType": "pull_request",
                    "resourceId": f"{owner}/{repo}#{number}", "title": title, "status": "open", "statusLabel": "Open",
                    "metadataJson": json.dumps(meta), "isDismissed": False, "isTerminal": False, "relationship": "created",
                    "enrichmentStatus": "resolved", "createdAt": "2026-11-19T09:00:00Z", "updatedAt": "2026-11-19T09:00:00Z",
                }
            )
    s = Standin(fixture, a.fleet, links, a.repos_delay, a.repos_fail)
    srv = socket.create_server(("127.0.0.1", a.listen))
    while True:
        conn, _ = srv.accept()
        threading.Thread(target=s.handle, args=(conn,), daemon=True).start()


if __name__ == "__main__":
    main()
