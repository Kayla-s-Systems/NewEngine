# newengine-assets

Engine-side контракт для **AssetManager** (как сервис) + небольшой protocol-first client.

## Ответственность

- `AssetServiceClient` — обращение к AssetManager через `HostApiV1::call_service_v1`.
- Минимальные трейты `AssetService/AssetAccess` для тестируемости и подмены.
- Константы контрактов: service id и имена методов.

## Не ответственность

- Не содержит реализацию AssetManager (она живёт в отдельном плагине).
- Не зависит от UI/рендера.

## Ссылки

- `../../ARCHITECTURE.md`
