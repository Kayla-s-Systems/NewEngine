# newengine-scene-authoring-runtime

Focused authored-scene transaction owner.

Owns authored placement dirty/create/delete journal, duplicate identity allocation, YMAP source patching and package-writer rebuild requests. It does not own SceneBridge, runtime camera/view/bootstrap, renderer, gameplay, or editor viewport policy. Scene composition passes a `World` and project root explicitly.
