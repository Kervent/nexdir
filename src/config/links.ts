// Web addresses Nexdir can open — always after a click, never in the background. The backend
// only opens addresses below the repository URL and Discord invites
// (`security::validate_external_url`), so changing these to anything else has no effect.

export const REPOSITORY_URL = "https://github.com/Kervent/nexdir";

export const LINKS = {
  repository: REPOSITORY_URL,
  issues: `${REPOSITORY_URL}/issues`,
  releases: `${REPOSITORY_URL}/releases`,
  contributors: `${REPOSITORY_URL}/graphs/contributors`,
  contributing: `${REPOSITORY_URL}/blob/main/CONTRIBUTING.md`,
  documentation: `${REPOSITORY_URL}/tree/main/docs`,
  security: `${REPOSITORY_URL}/security/advisories/new`,
  newBug: `${REPOSITORY_URL}/issues/new?template=bug_report.yml`,
  newFeature: `${REPOSITORY_URL}/issues/new?template=feature_request.yml`,
} as const;

const INVITE = /^https:\/\/(discord\.gg|discord\.com\/invite)\/[A-Za-z0-9-]{2,32}$/;

/**
 * Invite link to the official Nexdir Discord server, or `null` when this build has none.
 * Set it at build time with the environment variable `VITE_NEXDIR_DISCORD_INVITE`
 * (for example `https://discord.gg/abc123`); anything that is not a Discord invite is ignored.
 */
export function discordInvite(value: string | undefined = import.meta.env.VITE_NEXDIR_DISCORD_INVITE): string | null {
  const v = value?.trim();
  return v && INVITE.test(v) ? v : null;
}
