import type { ReactNode } from "react";

export function Card({
  id,
  title,
  subtitle,
  actions,
  children,
  className = "",
  flush = false,
}: {
  id?: string;
  title?: ReactNode;
  subtitle?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  flush?: boolean;
}) {
  return (
    <section id={id} className={`card ${className}`.trim()}>
      {title || actions ? (
        <header className="card-header">
          <div>
            {title ? <h2 className="card-title">{title}</h2> : null}
            {subtitle ? <p className="card-subtitle">{subtitle}</p> : null}
          </div>
          {actions ? <div className="card-actions">{actions}</div> : null}
        </header>
      ) : null}
      <div className={flush ? "card-body card-body-flush" : "card-body"}>{children}</div>
    </section>
  );
}
