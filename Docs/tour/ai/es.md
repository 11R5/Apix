El servidor respondió 404 Not Found: no existe la ruta /status en jsonplaceholder.typicode.com. La solicitud en sí está bien (el servidor respondió rápido y las cabeceras son normales), lo que falla es la dirección.

- Revisa la ruta: esta API tiene /users, /posts y /comments, pero no /status.
- Si la página de estado está en otro servidor, pon ese servidor en base_url para esta solicitud o en su entorno.
- La comprobación "status == 200" falló solo por el 404; pasará cuando la dirección sea la correcta.
