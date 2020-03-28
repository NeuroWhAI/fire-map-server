FROM postgres

ENV POSTGRES_PASSWORD=postgres
ADD setup.sql /docker-entrypoint-initdb.d/
