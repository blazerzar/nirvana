use leptos::prelude::*;

/// The sidebar and the content pane around a page.
#[component]
pub(super) fn Shell(#[prop(optional)] children: Option<Children>) -> impl IntoView {
    view! {
        <div class="shell">
            <Sidebar />
            <main class="content">{children.map(|children| children())}</main>
        </div>
    }
}

#[component]
fn Sidebar() -> impl IntoView {
    view! {
        <nav class="sidebar">
            <div class="brand">
                <img src="/logo.svg" width="24" height="24" alt="" />
                <span class="wordmark">"nirvana"</span>
                <span class="tag">"web"</span>
            </div>
            <div class="nav">
                <NavItem label="Timesheet" />
                <NavItem label="Tickets" />
                <NavItem label="Profiles & connections" />
                <NavItem label="Settings" />
            </div>
        </nav>
    }
}

#[component]
fn NavItem(label: &'static str) -> impl IntoView {
    view! { <a class="nav-item">{label}</a> }
}
