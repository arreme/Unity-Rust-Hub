import type { ReactNode } from "react";

type Tone = "neutral" | "green" | "amber" | "red" | "blue" | "orange" | "violet";

export function Badge({
  children,
  tone = "neutral",
  mono = false,
}: {
  children: ReactNode;
  tone?: Tone;
  mono?: boolean;
}) {
  return (
    <span className={`badge badge-${tone} ${mono ? "badge-mono" : ""}`.trim()}>
      {children}
    </span>
  );
}
