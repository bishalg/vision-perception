# Clinic server

owner: WP-B.18 (DPL-1..11, ADR-0011 D8)

One server runs everything a clinic needs: the console (web), the API the iPads upload to,
the database, the file store, nightly encrypted backups and a monthly restore test. It is
installed with one command. This is the default production deployment;
`rust/docker-compose.yml` stays the developer/demo stack and the AWS checklist
(`docs/security/aws-baa-checklist.md`) applies only if this stack is ever hosted on AWS.

## What you need

- A computer that stays on (a small PC or server is enough: 4 GB RAM, 2 cores, 100 GB
  disk), with **Ubuntu 24.04 or Debian 12 installed with disk encryption** ("Encrypt the
  new installation" / LUKS in the installer). The installer refuses an unencrypted disk.
- Docker Engine 25 or newer with the compose plugin:
  <https://docs.docker.com/engine/install/ubuntu/>.
- A name for the server, such as `clinic.example.org`, and `api.clinic.example.org` pointing
  at the same machine. iPads cannot use a bare IP address.
- This folder (from the release, or the source checkout).

## Install

```sh
sudo ./install.sh
```

It asks four questions:

1. the web address (for example `clinic.example.org`);
2. the certificate mode: `letsencrypt`, `internal` or `tunnel` (next section);
3. the administrator's e-mail (this becomes the owner's sign-in);
4. the clinic's name.

Then it creates every password and key, starts the server, creates the clinic and the
owner account, schedules the backups and the restore test, and prints the **key card**
once. **Photograph it or copy it onto paper before you close the window, and keep it
locked away from the server.** It is never stored anywhere else. Without it, backups
cannot be restored and clinical notes cannot be read.

Running `sudo ./install.sh` again later is safe: it changes nothing and shows the status.
An installation that stopped half-way (power cut, network) is finished by running it again.

For automation: `sudo ./install.sh --non-interactive --domain clinic.example.org --tls
letsencrypt --admin-email it@example.org --org-name "Example Clinic"` (`--help` lists all
options).

### Key card template

The installer prints this; the values below are examples. Write the real ones down.

```
================================ KEY CARD =================================
 Example Clinic - clinic.example.org - installed 2026-11-02
 Console:            https://clinic.example.org
 Device address:     https://api.clinic.example.org
 Owner sign-in:      it@example.org
 Owner password:     abcde-fghjk-mnpqr-stuvw   (set up the authenticator within 14 days)
 DATA KEY:           (44 characters, ends with =)       opens clinical notes
 BACKUP KEY:         AGE-SECRET-KEY-1...                opens the backups
 Bundle public key:  (43 characters)                    for the iPad app configuration
===========================================================================
```

## TLS modes

- **letsencrypt** - the server is reachable from the internet on ports 80 and 443 and both
  names resolve to it publicly. Caddy gets and renews free certificates by itself. Best for
  iPads: nothing to install on them.
- **internal** - the server is only on the clinic network. Caddy makes its own certificate
  authority. Two extra steps:
  1. **A name on the clinic network.** In the router (or Pi-hole) add local DNS entries
     `clinic.example.org` and `api.clinic.example.org` -> the server's LAN address. An IP
     address is not accepted by the iPad app.
  2. **Trust the certificate on every iPad.** The installer prints the file
     `/var/lib/asd/caddy-root.crt`. Send it to the iPad (AirDrop or e-mail), open it,
     then *Settings -> General -> VPN & Device Management* -> install the profile, and then
     *Settings -> General -> About -> Certificate Trust Settings* -> switch it **on**.
     Without the last switch every upload fails with a certificate error.
- **tunnel** - no open ports: Cloudflare Tunnel carries the traffic. Create a tunnel in the
  Cloudflare dashboard, add the public hostnames `clinic.example.org` and
  `api.clinic.example.org`, both to the service `http://caddy:80`, and give the installer
  the tunnel token (it asks).

Only Caddy has open ports. The database and the file store are never reachable from
outside the server. HSTS is sent by the API and the console themselves.

## Everyday

```sh
sudo ./status.sh        # is everything fine? (also shows the last backup and restore test)
sudo ./backup.sh        # a backup now (the timer does this every night at 02:30)
```

`status.sh` exits non-zero and prints `WARN` lines when something needs attention: the
server is not ready, the last backup failed or is older than a day, or the last restore
test failed or is older than 45 days.

## Backups

Every night at 02:30 `backup.sh` writes `/var/lib/asd/backups/asd-<date>.tar.age`: the
database, every uploaded file, the erasure ledger and a manifest with row counts and
checksums, encrypted with the **public** backup key. The server cannot read its own
backups; only the BACKUP KEY on the key card can. The settings file `.env` (the secrets)
is deliberately not in the backup: the key card is the backup of the keys.

14 daily and 8 weekly backups are kept (`BACKUP_KEEP_DAILY`, `BACKUP_KEEP_WEEKLY` in
`.env`). For an off-site copy set `BACKUP_REMOTE` to an rclone remote (for example
`offsite:clinic-backups`) and put its configuration in `/var/lib/asd/rclone.conf`; only the
encrypted files leave the server. A failed backup writes `backups/LAST_FAILED`.

## Restore

```sh
sudo ./restore.sh /var/lib/asd/backups/asd-20261102T023000Z.tar.age
```

It asks for the BACKUP KEY (paste the `AGE-SECRET-KEY-1...` line; or `--key-file FILE`),
asks you to type `RESTORE`, then: checks the archive, stops the API, replaces the database
and the files with the backup, upgrades the database to this version if the backup is
older, **erases again everyone who was erased after the backup** (the erasure ledger), starts
everything and prints the row counts next to the backup's.

- Everything recorded after the backup was made is lost.
- On a **new server** (the old one is gone): install it first with the same web address,
  then open `.env` (`sudo nano .env`) and replace the value of `ASD_DATA_KEK` with the DATA
  KEY from the key card, then run `restore.sh`. Without the old data key the clinical
  notes in the backup cannot be read; `restore.sh` refuses to run if `ASD_DATA_KEK` is
  missing. The owner password is the one from the old key card (it is in the backup).

## Restore drill

`restore-drill.sh` runs on the first Sunday of every month at 03:30. It restores the newest
backup into a separate, temporary copy (nothing is published, the live server is not
touched), checks the row counts against the manifest, checks that the DATA KEY in `.env`
still opens the clinical data (`asd-backend admin verify`), deletes the copy and writes
`/var/lib/asd/drills/<date>.PASS` or `.FAIL` with a log.

To run unattended it needs the BACKUP KEY on the server: save the `AGE-SECRET-KEY-1...`
line in `/etc/asd/backup-key.txt` (`sudo nano /etc/asd/backup-key.txt`, then
`sudo chmod 600 /etc/asd/backup-key.txt`). Trade-off: someone who steals the server can
then read the local backups too, but they could read the live database anyway; the
off-site copies stay protected as long as the server itself is safe. If you prefer not to
keep the key on the server, run the drill yourself every month with
`sudo ./restore-drill.sh --key-file /media/usb/backup-key.txt`; until then the scheduled
drill records FAIL ("no backup key") and `status.sh` warns.

## Upgrading

```sh
sudo ./upgrade.sh 0.2.0            # images from the registry (ASD_IMAGE_PREFIX)
sudo ./upgrade.sh 0.2.0 --build    # or built from this source checkout
```

It makes a backup first and does not continue if that fails, switches `ASD_VERSION`,
upgrades the database, restarts and prints the version the API reports. If the database
step fails, `.env` goes back to the old version. Notes for specific versions:

- **Any version after 0.1.0 from a database older than migration 0022**: review notes
  stored before migration 0008 must be encrypted first with `asd-backend admin
  encrypt-review-notes` of the previous release (`rust/README.md`, "Upgrading"). A fresh
  installation from this folder never has them.

## Running it on a cloud server

The same folder works on a rented virtual machine (Hetzner, AWS EC2, etc.): an Ubuntu
24.04 VM with an encrypted volume for `/var/lib/docker` and `/var/lib/asd`, ports 80/443
open, DNS for both names, `TLS_MODE=letsencrypt`. Where the server (and the off-site
backup copy) may be located is an owner decision on data residency and the provider's
agreement (O-7, O-8); do not put patient data on a cloud VM before that decision.

## For developers

- Files: `docker-compose.yml` (services `caddy`, `api`, `worker`, `migrate`, `postgres`,
  `minio`, `minio-init`, `console`, `backup` (profile), `tunnel` (profile)), `Caddyfile`,
  `.env.example` (every setting), `backup.Dockerfile`, `lib.sh` (shared helpers),
  `systemd/` (timers, rendered by `install.sh`).
- `api` and `worker` connect as `asd_app` (DML only, append-only `audit_log`); `migrate`,
  the admin commands and the backup connect as the owner role.
- `test/compose_smoke.sh` is the definition of done (CI `deploy.yml`): `bash
  deploy/clinic-server/test/compose_smoke.sh`. It installs into a throwaway project
  (`asd-smoke`, ports 18080/18443) and tears it down.
- The restore scripts use `asd-backend admin erasure-ledger`, `requeue-erasure` and
  `verify` (WP-B.22). With an older backend image they print `SKIPPED (B.22 pending)` for
  those steps.
