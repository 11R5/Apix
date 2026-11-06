Le serveur a répondu 404 Not Found : il n’y a pas de chemin /status sur jsonplaceholder.typicode.com. La requête elle-même est correcte (le serveur a répondu vite et les en-têtes sont normaux) ; c’est l’adresse qui ne va pas.

- Vérifiez le chemin : cette API a /users, /posts et /comments, mais pas /status.
- Si la page d’état est sur un autre serveur, mettez-le dans base_url pour cette requête ou dans son environnement.
- La vérification « status == 200 » a échoué uniquement à cause du 404 ; elle passera avec la bonne adresse.
