//! Isolated #60 Leptos lifecycle experiment (doc 04 §3, ADR-011).
//!
//! This example mounts no production route, network client, auth context or rules.
//! The JS controller consumes only Rust-exported permitted presentation fixtures.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = tabulaSpikeAction)]
    fn spike_action(action: &str);
}

#[component]
fn RendererEmbeddingShell() -> impl IntoView {
    view! {
        <main>
            <h1>"Renderer embedding experiment"</h1>
            <p>
                "Isolated Leptos example. Rust owns permitted fixtures and rules; "
                "this shell owns mounting, preferences, focus and the modal. "
                "No production route or match socket is active."
            </p>
            <div class="controls">
                <button on:click=move |_| spike_action("mount:iframe")>
                    "Mount Macroquad iframe"
                </button>
                <button on:click=move |_| spike_action("mount:pixi")>
                    "Mount Pixi canvas"
                </button>
                <button on:click=move |_| spike_action("dispose")>"Unmount"</button>
                <button on:click=move |_| spike_action("focus")>"Focus canvas"</button>
                <button on:click=move |_| spike_action("modal")>"Toggle shell modal"</button>
                <button on:click=move |_| spike_action("motion")>"Toggle reduced motion"</button>
                <a href="macroquad.html">"Macroquad separate document control"</a>
            </div>
            <output id="spike-status" aria-live="polite">"Loading experiment modules…"</output>
            <div id="renderer-container" aria-label="Isolated permitted projection rendering surface"></div>
        </main>
        <section id="spike-modal" hidden aria-label="Shell modal" role="dialog" aria-modal="true">
            <h2>"Shell retains modal focus"</h2>
            <button on:click=move |_| spike_action("modal")>"Return to canvas"</button>
        </section>
    }
}

fn main() {
    leptos::mount::mount_to_body(RendererEmbeddingShell);
}
