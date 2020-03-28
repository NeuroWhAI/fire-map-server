# fire-map-server

Server that helps cope with large fires with collective intelligence.

## DB setup

1. Build `DB.Dockerfile`.
> docker build -t firemap-db -f DB.Dockerfile . 

1. Run a container for DB.
> docker run -d -p 5432:5432 --name fmap-main-db -e POSTGRES_PASSWORD=`$your_pwd` firemap-db

1. Set environment variable `DATABASE_URL`.
> export DATABASE_URL=postgresql://postgres:`$your_pwd`@localhost:5432/postgres
