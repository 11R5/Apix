服务器返回了 404 Not Found：jsonplaceholder.typicode.com 上没有 /status 这个路径。请求本身没有问题（服务器很快就响应了，请求头也正常），出错的是地址。

- 检查路径：这个 API 有 /users、/posts 和 /comments，但没有 /status。
- 如果状态页在另一台主机上，请在此请求或其环境的 base_url 中填写那台主机。
- “status == 200”检查失败只是因为 404；地址正确后就会通过。
