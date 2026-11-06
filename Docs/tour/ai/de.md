Der Server hat mit 404 Not Found geantwortet: Auf jsonplaceholder.typicode.com gibt es keinen Pfad /status. Die Anfrage selbst ist in Ordnung (der Server hat schnell geantwortet, die Header sind normal), falsch ist die Adresse.

- Prüfen Sie den Pfad: Diese API hat /users, /posts und /comments, aber kein /status.
- Liegt die Statusseite auf einem anderen Host, tragen Sie ihn für diese Anfrage oder in ihrer Umgebung als base_url ein.
- Die Prüfung "status == 200" ist nur wegen der 404 fehlgeschlagen; mit der richtigen Adresse besteht sie.
