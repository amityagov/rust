-- Schema for the `locks` crate: one table and the two functions it calls.
-- Safe to run more than once.

create table if not exists public.locks
(
    name        text                     not null primary key,
    owner_id    text                     not null,
    acquired_at timestamp with time zone not null,
    expires_at  timestamp with time zone not null
);

create index if not exists idx_locks_expires_at on public.locks (expires_at);

-- Deletes the lock if `lock_owner` holds it; returns whether it did.
create or replace function public.release_lock(lock_name text, lock_owner text) returns boolean
    language plpgsql
as
$$
declare
    released boolean;
begin
    with deleted as (
        delete from public.locks
            where name = lock_name
                and owner_id = lock_owner
            returning 1)
    select true
    into released
    from deleted;

    return coalesce(released, false);
end;
$$;

-- Takes the lock if it is free, expired or already held by `lock_owner` (which then extends it);
-- returns whether the caller holds it afterwards. The check and the write are one statement, so
-- concurrent callers cannot both succeed.
create or replace function public.acquire_lock(lock_name text, lock_owner text, ttl_seconds bigint) returns boolean
    language plpgsql
as
$$
declare
    got_it boolean;
begin
    with lock_attempt as (
        insert into public.locks (name, owner_id, acquired_at, expires_at)
            values (lock_name, lock_owner, now(), now() + make_interval(secs => ttl_seconds))
            on conflict (name) do update
                set owner_id = excluded.owner_id,
                    acquired_at = case
                                      when public.locks.owner_id = excluded.owner_id then public.locks.acquired_at
                                      else excluded.acquired_at
                        end,
                    expires_at = excluded.expires_at
                where public.locks.expires_at <= now()
                    or public.locks.owner_id = excluded.owner_id
            returning 1)
    select true
    into got_it
    from lock_attempt;

    return coalesce(got_it, false);
end;
$$;
