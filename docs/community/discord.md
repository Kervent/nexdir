# Nexdir Community on Discord

The server structure is defined in [`tools/discord-setup/config/server.toml`](../../tools/discord-setup/config/server.toml)
and applied with the setup tool ([README](../../tools/discord-setup/README.md)).

## Roles

| Role | Purpose | Server permissions (beyond members) |
|---|---|---|
| Founder | Project owner, final organisational decisions. Assigned **only** to the server owner. | Server, roles, channels, webhooks, events, moderation |
| Administrator | Trusted server administration | Same as Founder (no Administrator flag; the owner may add it manually) |
| Moderator | Community moderation, spam, timeouts | Kick, ban, timeout, manage messages/threads/nicknames, audit log |
| Core Developer | Technical lead, reviews, release coordination | Manage threads, pin messages; can post in announcements, releases, roadmap |
| Developer | Regular code contributors | Manage threads |
| Support Team | Helps in support and bug reports | Manage threads, pin messages |
| Contributor | Recognised open-source contributors | — |
| Beta Tester | Tests pre-releases | — |
| Member | Community member | — |
| Release Notifications / Beta Updates / Development News | Opt-in pings | — (mentionable) |
| Interested: Development / Beta Testing / Community Support / Documentation / Translation / Moderation | Self-selected interests | — |

Project roles and server rights are separate: becoming a Developer never grants server
administration. Team roles are assigned manually after an application is accepted; nothing is
promoted automatically. Responsibilities, GitHub access and the application process are in
[team-roles.md](team-roles.md).

## Automated announcements

#releases, #github-updates and #roadmap receive posts from GitHub through channel webhooks
(no bot). What is posted, and how to set it up: [tools/discord-notify](../../tools/discord-notify/README.md).

## Private team area

| Channel | Visible to |
|---|---|
| #team-general | Founder, Administrator, Moderator, Core Developer, Developer, Support Team |
| #developer-internal | Founder, Administrator, Core Developer, Developer |
| #support-internal | Founder, Administrator, Moderator, Support Team |
| #moderation | Founder, Administrator, Moderator |
| #staff-applications | Founder, Administrator, Core Developer |
| #mod-logs | Founder, Administrator (write), Moderator (read) |

Public development decisions belong on GitHub; private channels are only for things that must
stay confidential (e.g. security fixes before disclosure, moderation cases, applications).
