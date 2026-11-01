import { Component, type ErrorInfo, type ReactNode } from "react";

interface State {
  error: Error | null;
}

/** Last line of defence: shows a recovery screen instead of a blank window. */
export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Nexdir UI error", error, info.componentStack);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="nx-state" role="alert" style={{ height: "100%" }}>
        <h2>Something went wrong</h2>
        <p>Nexdir hit an unexpected problem in its interface. No file operation was affected by this.</p>
        <p className="nx-state-path">{this.state.error.message}</p>
        <div className="nx-state-actions">
          <button type="button" className="nx-button is-primary" onClick={() => window.location.reload()}>
            Reload Nexdir
          </button>
        </div>
      </div>
    );
  }
}
