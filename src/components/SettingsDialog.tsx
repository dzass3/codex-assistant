import { useEffect, useState } from "react";
import type { MonitorSettings } from "../../shared/monitor-types";

interface SettingsDialogProps {
  open: boolean;
  settings: MonitorSettings | null;
  onClose: () => void;
  onSave: (path: string) => Promise<void>;
}

export function SettingsDialog({ open, settings, onClose, onSave }: SettingsDialogProps) {
  const [path, setPath] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setPath("");
      setError(null);
    }
  }, [open]);

  if (!open) return null;

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      await onSave(path.trim());
      onClose();
    } catch {
      setError("该目录不是有效的 Codex Home，请检查其中是否包含 state_5.sqlite 或 sessions。");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        className="settings-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <button className="dialog-close" aria-label="关闭" onClick={onClose}>
          ×
        </button>
        <span className="eyebrow">PRIVACY & SOURCE</span>
        <h2 id="settings-title">监控数据目录</h2>
        <p className="dialog-copy">
          默认自动读取当前 Windows 用户的 Codex
          Home。设置自定义目录时，只保存目录位置，不会修改其中任何文件。
        </p>
        <div className="current-location">
          <span>当前来源</span>
          <strong>{settings?.codex_home_label ?? "检测中"}</strong>
        </div>
        <label className="path-field">
          <span>自定义 Codex Home</span>
          <input
            value={path}
            onChange={(event) => setPath(event.target.value)}
            placeholder="例如 D:\\CodexHome"
            autoFocus
          />
        </label>
        <p className="field-help">留空保存可恢复默认自动检测。</p>
        {error && <p className="dialog-error">{error}</p>}
        <div className="dialog-actions">
          <button className="button-secondary" onClick={onClose}>
            取消
          </button>
          <button className="button-primary" onClick={save} disabled={saving}>
            {saving ? "验证中…" : "验证并保存"}
          </button>
        </div>
      </section>
    </div>
  );
}
