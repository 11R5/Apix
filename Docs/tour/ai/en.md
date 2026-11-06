The server answered 404 Not Found: there is no /status path on jsonplaceholder.typicode.com. The request itself is fine (the host answered quickly and the headers are normal), so the address is what is wrong.

- Check the path: this API has /users, /posts and /comments, but no /status.
- If the status page lives on another host, put that host in base_url for this request or in its environment.
- The check "status == 200" failed only because of the 404; it will pass once the address is right.
