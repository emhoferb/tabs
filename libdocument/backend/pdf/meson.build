# The core poppler library, not just its glib wrapper: the animate-package
# support needs form field icons, which the glib API does not expose.
poppler_core_dep = dependency('poppler', version: poppler_req_version)

backend_deps = backends_common_deps + [
  cairo_pdf_dep,
  cairo_ps_dep,
  poppler_glib_dep,
  poppler_core_dep,
]

backend_sources = files(
  'pps-poppler.c',
  'pps-pdf-animation.cc',
)
