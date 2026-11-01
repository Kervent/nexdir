import { Dialog } from "../../components/Dialog";
import logo from "../../../assets/branding/png/nexdir-logo-128.png";

/** Version and project facts — the version is what bug reports ask for. */
export function AboutDialog({ onClose }: { onClose: () => void }) {
  return (
    <Dialog
      title="About Nexdir"
      onClose={onClose}
      footer={
        <button type="button" className="nx-button is-primary" onClick={onClose}>
          Close
        </button>
      }
    >
      <div className="nx-about">
        <img src={logo} width={64} height={64} alt="" />
        <div>
          <p className="nx-about-name">Nexdir</p>
          <p className="nx-about-version">Version {__APP_VERSION__}</p>
          <p>A smarter way to manage your files.</p>
        </div>
      </div>
      <ul className="nx-about-facts">
        <li>Open source under the MIT License.</li>
        <li>No telemetry, no accounts, no network access. Your files and file names stay on this PC.</li>
        <li>Nexdir never deletes files permanently — removed items go to the Recycle Bin.</li>
        <li>Report bugs at github.com/Kervent/nexdir/issues and include the version above.</li>
      </ul>
    </Dialog>
  );
}
