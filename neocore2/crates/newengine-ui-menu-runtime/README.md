# newengine-ui-menu-runtime

Generic menu runtime for declarative `MenuDocument` data.

It owns navigation state, selection movement, hover/click activation and route dispatch. It does not execute engine side effects; callers receive `MenuActionRoute` events and pass them to a command router.
