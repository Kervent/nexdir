"""Computes the changes needed to bring a server in line with the configuration.

Rules:
* Missing roles, categories, channels and forum tags are created.
* Nothing is ever deleted. Extra roles/channels on the server are left alone.
* Differences on items that already exist are listed as updates (`existing=True`). They
  are applied only when the user explicitly allows updates.
* Updating overwrites merges: only the entries this tool manages are replaced; overwrites
  for other roles or members stay as they are.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from .config import CategorySpec, ChannelSpec, RoleSpec, ServerSpec
from .model import ChannelState, ServerState
from .permissions import Overwrite, category_overwrites, channel_overwrites, required_bot_permissions


@dataclass
class Change:
    kind: str
    target: str
    description: str
    existing: bool = False
    data: dict = field(default_factory=dict)

    @property
    def key(self) -> str:
        return f"{self.kind}:{self.target}"


@dataclass
class Plan:
    changes: list[Change] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)
    blockers: list[str] = field(default_factory=list)
    missing_bot_permissions: frozenset[str] = frozenset()

    @property
    def creations(self) -> list[Change]:
        return [c for c in self.changes if not c.existing]

    @property
    def updates(self) -> list[Change]:
        return [c for c in self.changes if c.existing]

    @property
    def is_empty(self) -> bool:
        return not self.changes


def _overwrites_differ(current: dict[str, Overwrite], desired: dict[str, Overwrite]) -> bool:
    """Only the desired keys are compared; unrelated overwrites are not our business."""
    return any(current.get(k) != v for k, v in desired.items())


def _channel_kind(spec: ServerSpec, state: ServerState, ch: ChannelSpec) -> str | None:
    if ch.type == "forum" and not state.community:
        return None if spec.forum_fallback == "skip" else "text"
    return ch.type


def plan_changes(spec: ServerSpec, state: ServerState) -> Plan:
    plan = Plan()

    # --- Bot permissions ------------------------------------------------------------
    missing = required_bot_permissions(spec) - state.bot_permissions
    if "administrator" not in state.bot_permissions and missing:
        plan.missing_bot_permissions = frozenset(missing)
        plan.blockers.append(
            "The bot lacks permissions it needs to create these roles and channels: "
            + ", ".join(sorted(missing))
            + ". Re-invite it with the URL from 'Check connection'."
        )
    if "administrator" in state.bot_permissions:
        plan.warnings.append("The bot has the Administrator permission. It does not need it; consider removing it after the setup.")

    # --- Server ---------------------------------------------------------------------
    if state.name != spec.name:
        plan.changes.append(Change("server_name", "server", f'Rename server "{state.name}" → "{spec.name}"', existing=True, data={"name": spec.name}))
    if spec.description and state.description != spec.description:
        if state.community:
            plan.changes.append(Change("server_description", "server", "Set server description", existing=bool(state.description), data={"description": spec.description}))
        else:
            plan.warnings.append("The server description can only be set on Community servers. Enable Community first (manual step).")

    if not state.community and any(ch.type == "forum" for _, ch in spec.channels()):
        action = "created as text channels" if spec.forum_fallback == "text" else "skipped"
        plan.warnings.append(f"Community is not enabled, so forum channels will be {action}. Enable Community and run the setup again to get forums.")

    # --- Roles ----------------------------------------------------------------------
    for role in spec.roles:
        existing = state.role(role.name)
        if existing is None:
            plan.changes.append(Change("create_role", role.name, f'Create role "{role.name}"', data={"role": role}))
            continue
        if existing.managed:
            plan.warnings.append(f'Role "{role.name}" belongs to an integration and cannot be changed.')
            continue
        if existing.position >= state.bot_top_role_position:
            plan.warnings.append(f'Role "{role.name}" is above the bot\'s role and cannot be changed by the bot.')
            continue
        diffs = _role_diffs(role, existing.permissions, existing.color, existing.hoist, existing.mentionable)
        if diffs:
            plan.changes.append(
                Change("update_role", role.name, f'Update role "{role.name}": ' + ", ".join(diffs), existing=True, data={"role": role, "role_id": existing.id})
            )

    order = _role_order_change(spec, state)
    if order:
        plan.changes.append(order)

    # --- Categories and channels ----------------------------------------------------
    for cat in spec.categories:
        desired_cat = category_overwrites(cat).normalized()
        cat_state = state.category(cat.name)
        if cat_state is None:
            plan.changes.append(Change("create_category", cat.name, f'Create category "{cat.name}"' + (" (private)" if cat.private else ""), data={"category": cat}))
        elif _overwrites_differ(cat_state.overwrites, desired_cat):
            plan.changes.append(
                Change("update_category", cat.name, f'Update permissions of category "{cat.name}"', existing=True, data={"category": cat, "channel_id": cat_state.id})
            )
        for ch in cat.channels:
            _plan_channel(spec, state, plan, cat, ch, cat_state)
    return plan


def _role_diffs(role: RoleSpec, permissions: frozenset[str], color: int, hoist: bool, mentionable: bool) -> list[str]:
    diffs = []
    if permissions != role.permissions:
        added = role.permissions - permissions
        removed = permissions - role.permissions
        parts = []
        if added:
            parts.append("+" + " +".join(sorted(added)))
        if removed:
            parts.append("-" + " -".join(sorted(removed)))
        diffs.append("permissions " + " ".join(parts))
    if role.color is not None and color != role.color:
        diffs.append("colour")
    if hoist != role.hoist:
        diffs.append("shown separately" if role.hoist else "not shown separately")
    if mentionable != role.mentionable:
        diffs.append("mentionable" if role.mentionable else "not mentionable")
    return diffs


def _role_order_change(spec: ServerSpec, state: ServerState) -> Change | None:
    """Arranges configured roles in config order directly below the bot's top role."""
    names = [r.name for r in spec.roles]
    existing = [state.role(n) for n in names]
    present = [r for r in existing if r is not None]
    movable = [r for r in present if not r.managed and r.position < state.bot_top_role_position]
    current_order = [r.name.lower() for r in sorted(movable, key=lambda r: -r.position)]
    desired_order = [n.lower() for n in names if (r := state.role(n)) is not None and r in movable]
    any_missing = len(present) < len(names)
    if current_order == desired_order and not any_missing:
        return None
    return Change(
        "order_roles",
        "roles",
        "Arrange roles in the configured order (highest first)",
        # Moving pre-existing roles changes the existing hierarchy.
        existing=current_order != desired_order and len(movable) > 1,
        data={"order": names},
    )


def _plan_channel(spec: ServerSpec, state: ServerState, plan: Plan, cat: CategorySpec, ch: ChannelSpec, cat_state: ChannelState | None) -> None:
    kind = _channel_kind(spec, state, ch)
    if kind is None:
        plan.warnings.append(f'Forum "#{ch.name}" skipped (Community not enabled).')
        return
    desired = channel_overwrites(cat, ch).normalized()
    existing = state.channel(ch.name)
    if existing is None:
        label = "forum" if kind == "forum" else "channel"
        plan.changes.append(Change("create_channel", ch.name, f'Create {label} "#{ch.name}" in {cat.name}', data={"category": cat, "channel": ch, "kind": kind}))
        if kind == "forum":
            for tag in ch.tags:
                plan.changes.append(Change("create_tag", f"{ch.name}/{tag}", f'Add tag "{tag}" to #{ch.name}', data={"channel": ch.name, "tag": tag}))
        return
    if existing.type != kind:
        plan.warnings.append(f'"#{ch.name}" exists as a {existing.type} channel, expected {kind}. It is left unchanged; convert or rename it manually.')
        return
    updates: dict = {}
    reasons = []
    if cat_state is not None and existing.category_id != cat_state.id:
        updates["move"] = True
        reasons.append(f"move to {cat.name}")
    elif cat_state is None:
        updates["move"] = True
        reasons.append(f"move to new category {cat.name}")
    if ch.topic and existing.topic != ch.topic:
        updates["topic"] = ch.topic
        reasons.append("topic")
    if _overwrites_differ(existing.overwrites, desired):
        updates["overwrites"] = True
        reasons.append("permissions")
    if updates:
        plan.changes.append(
            Change("update_channel", ch.name, f'Update "#{ch.name}": ' + ", ".join(reasons), existing=True, data={"category": cat, "channel": ch, "channel_id": existing.id, **updates})
        )
    if kind == "forum":
        have = {t.lower() for t in existing.tags}
        for tag in ch.tags:
            if tag.lower() not in have:
                plan.changes.append(Change("create_tag", f"{ch.name}/{tag}", f'Add tag "{tag}" to #{ch.name}', data={"channel": ch.name, "tag": tag}))
