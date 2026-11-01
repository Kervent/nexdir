"""Posts GitHub events (releases, selected issues and pull requests, milestones, roadmap)
to the Nexdir Discord server through channel webhooks.

Separate from ``tools/discord-setup``: the setup tool can be removed after the server is
configured, and announcements keep working. Standard library only, so the workflow that
runs it installs nothing from the network.
"""

__version__ = "0.1.0"
