SELECT floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint AS "now_ms!"
