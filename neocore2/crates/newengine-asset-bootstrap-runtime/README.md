# newengine-asset-bootstrap-runtime

Asset/content-root bootstrap policy shared by product compositions and scene runtimes.
It lives outside `newengine-runtime-host` so lower domain runtimes do not depend upward
on process orchestration merely to resolve or mount content sets.
