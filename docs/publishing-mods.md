# Publishing a downloadable mod

A catalog entry becomes installable in the launcher (in-app install, #74) when it carries a `download` block: `{ url, size, sha256 }`. This guide shows how to add one. Only list a file when you are the author or have the author's written permission to redistribute it. Without that, leave `download` out: the launcher then just links to `pageUrl`.

## For creators

1. **Host the file on a GitHub Release of your own repository.** Upload the exact file you want users to install. The direct URL has the form `https://github.com/<owner>/<repo>/releases/download/<tag>/<file>`. The URL must be `https` and must not change afterwards.
2. **Generate the entry** with the script. It streams the file (large mods are fine), computes the size and the lowercase SHA-256, validates the entry against `catalog/catalog.schema.json` and prints it:

   ```
   corepack pnpm catalog-entry -- --file ./livery.zip \
     --url https://github.com/<owner>/<repo>/releases/download/v1.0/livery.zip \
     --id my-livery --title "My Livery" --author "Your Name" --game "MotoGP 26" \
     --category liveries --description "What it does." --version 1.0 \
     --page-url https://www.overtake.gg/downloads/my-livery.12345/ --verify
   ```

   - Optional: `--thumbnail-url <https-url>` and `--updated-at YYYY-MM-DD` (default: today, UTC).
   - `--verify` downloads the hosted URL (following redirects, as GitHub Release assets do) and checks that its size and SHA-256 match the local file. On a mismatch it exits with code 1 and prints which one differs. Always use it before opening the PR.
   - Without the `corepack pnpm` wrapper: `node scripts/catalog-entry.mjs <same flags>` (Node 22).
3. **Add the printed entry** to `entries` in `catalog/catalog.json`.
4. **Open a pull request** to `dev` with the "Add a mod" template: append `?template=add-a-mod.md` to the PR creation URL. Tick the permission checkbox and fill in the permission record.

Cannot or do not want to host the file yourself? Say so in the PR ("please mirror") and we will mirror it, see below.

## Mirroring a file (maintainers)

Do this only with the author's **written permission** to redistribute the file.

1. Upload the exact file, unmodified, to a GitHub Release of this repository tagged `mods-<id>-<version>`. Make it a **prerelease**, so it never shadows the app's `v<version>` release or the `latest.json` used by auto-update:

   ```
   gh release create mods-<id>-<version> ./<file> --prerelease \
     --title "<mod title> <version>" --notes "Mirrored with the author's permission. Page: <pageUrl>"
   ```

2. Run the script with `--verify` against the asset URL (`https://github.com/IcaroSilvaFK/motogp-launcher/releases/download/mods-<id>-<version>/<file>`).
3. Record the permission (link or quote, and the date) in the PR description.

## Permission request message

Ready to send to a mod author.

**English**

> Hi <author name>,
>
> I maintain MotoGP Launcher, a free, open-source launcher that lists community mods. I would like to list "<mod title>" in its catalog so players can install it with one click. The launcher would download the exact file you published and verify its SHA-256 before installing, and it always links back to your page: <page URL>.
>
> May I list and redistribute this file (hosted on a GitHub Release, by you or by us)? A short written "yes" is enough, and you can ask me to remove it at any time. Thank you for your work!

The next message is intentionally in Portuguese: it is meant to be sent to Portuguese-speaking authors, it is not project documentation.

**Português**

> Olá <nome do autor>,
>
> Eu mantenho o MotoGP Launcher, um launcher gratuito e de código aberto que lista mods da comunidade. Gostaria de listar "<título do mod>" no catálogo para que os jogadores possam instalá-lo com um clique. O launcher baixaria exatamente o arquivo que você publicou e conferiria o SHA-256 antes de instalar, sempre com link para a sua página: <URL da página>.
>
> Posso listar e redistribuir este arquivo (hospedado em um GitHub Release, por você ou por nós)? Um "sim" curto por escrito basta, e você pode pedir a remoção a qualquer momento. Obrigado pelo seu trabalho!
