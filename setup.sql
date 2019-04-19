create table reports (
	id serial not null primary key,
	user_id text not null,
	user_pwd text not null,
	latitude double precision not null,
	longitude double precision not null,
	created_time timestamp not null,
	lvl integer not null,
	description text,
	img_path text
);
create table bad_reports (
	id serial not null primary key,
	report_id integer not null,
	reason text
);