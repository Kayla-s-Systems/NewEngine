# newengine-startup-window-egui

Optional concrete Egui presenter for the NewEngine PreStart configuration surface.

`newengine-core` owns only the typed startup settings/report contract and a presenter registration port. Host/product compositions opt into this crate when they want a native Egui PreStart window. Empty/Void Engine hosts do not link a UI toolkit.
