import { Component, type ErrorInfo, type ReactNode } from "react";
import { t } from "../../i18n";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
}

export class ErrorBoundary extends Component<Props, State> {
  public state: State = { hasError: false };

  public static getDerivedStateFromError(): State {
    return { hasError: true };
  }

  public componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error("UI boundary captured an error", {
      error: error.message,
      componentStack: info.componentStack,
    });
  }

  public render(): ReactNode {
    if (this.state.hasError) {
      return (
        <main className="app-shell">
          <h1>{t("fa", "error.boundaryTitle")}</h1>
          <p>{t("fa", "error.boundaryBody")}</p>
        </main>
      );
    }

    return this.props.children;
  }
}
