//! 构建脚本：把 apps/desktop/icons/icon.ico 嵌入 cloudmount-desktop.exe 的资源段
//! （prd AC1：winres 方案嵌 exe；TP-16 RT_GROUP_ICON 枚举可验）。
//!
//! 首日验证② 选型定稿（留痕）：选 **winresource** 而非 winres——
//!   - winres 最后发版 0.1.12（2021-09-28），近 5 年无维护；
//!   - winresource 为其活跃维护继任（0.1.34，2026-10-12 发版），
//!     `WindowsResource` API 兼容，同 Cargo 集成语义（build-dependencies + compile()）。
//!
//! fail-fast：icon.ico 缺失/编译失败即 panic（build script 失败 → cargo build 失败 →
//! 上游 MSI/dmg 链路不产出，prd TP-12 图标管线 fail-fast 面）。
//!
//! 目标门控沿 `CARGO_CFG_TARGET_OS`（非 host cfg）：mac 目标构建时整体跳过
//! （mac 的 .app 图标走 icns，不嵌 exe）。

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=icons/icon.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/icon.ico");
        if let Err(e) = res.compile() {
            panic!("winresource 嵌入 icons/icon.ico 失败（图标缺失/工具链不可用——fail-fast）: {e}");
        }
    }
}
