// Vstupní nastavení hlavní stránky index.html, nikoliv seznam všech ukázek.
// Všech pět definic scén je v examples/scenes.js. Galerie je vybírá v gallery.js.
// setupScene se zavolá při startu stránky a po kliknutí na „Obnovit scénu“.
// Nevolá se každý snímek. loadExample spustí JS recept basic(scene), který
// přes SDK vytvoří objekty a světla ve SceneManageru v Rustu.
// Další snímky už zpracovává Rust: animace, kamera, culling, stíny a vykreslení.
export function setupScene(engine) {
    engine.scene.loadExample();
}
