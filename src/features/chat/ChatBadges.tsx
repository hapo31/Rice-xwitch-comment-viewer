import type { TwitchChatBadge } from "../../types";

interface BadgePresentation {
  accessibleName: string;
  label: string;
  className: string;
}

const knownBadges: Record<string, Omit<BadgePresentation, "accessibleName">> = {
  broadcaster: { label: "配信者", className: "bg-rose-500/20 text-rose-300" },
  moderator: { label: "モデ", className: "bg-emerald-500/20 text-emerald-300" },
  vip: { label: "VIP", className: "bg-fuchsia-500/20 text-fuchsia-300" },
  subscriber: { label: "購読", className: "bg-sky-500/20 text-sky-300" },
  staff: { label: "スタッフ", className: "bg-violet-500/20 text-violet-300" },
  admin: { label: "管理", className: "bg-rose-500/20 text-rose-300" },
  global_mod: { label: "全体モデ", className: "bg-emerald-500/20 text-emerald-300" },
  partner: { label: "パートナー", className: "bg-violet-500/20 text-violet-300" },
  bits: { label: "Bits", className: "bg-zinc-700 text-zinc-200" },
};

export function getBadgePresentation(badge: TwitchChatBadge): BadgePresentation {
  const knownBadge = knownBadges[badge.setId];
  if (knownBadge) {
    return { ...knownBadge, accessibleName: `${knownBadge.label}バッジ` };
  }

  return {
    label: badge.setId,
    accessibleName: `不明な Twitch バッジ: ${badge.setId}`,
    className: "bg-zinc-800 text-zinc-300",
  };
}

export function ChatBadges({ badges }: { badges?: TwitchChatBadge[] }) {
  if (!badges?.length) {
    return null;
  }

  return (
    <span className="flex min-w-0 shrink items-center gap-1" aria-label="Twitch バッジ">
      {badges.map((badge) => {
        const presentation = getBadgePresentation(badge);

        return (
          <span
            key={`${badge.setId}-${badge.id}`}
            role="img"
            aria-label={presentation.accessibleName}
            title={presentation.accessibleName}
            className={`max-w-20 truncate rounded px-1 py-0.5 text-[10px] font-semibold leading-none ${presentation.className}`}
          >
            {presentation.label}
          </span>
        );
      })}
    </span>
  );
}
