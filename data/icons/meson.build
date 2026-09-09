# The fork's own logo, as PNGs at the sizes it was drawn for. Installed
# instead of the upstream scalable icon rather than beside it: shipping both
# leaves the icon shown depending on which size the shell happens to ask for.
foreach size : ['16', '24', '32', '48', '64', '128', '256', '512']
  install_data(
    join_paths('@0@x@0@'.format(size), 'apps', '@0@.png'.format(app_id)),
    install_dir: join_paths(pps_datadir, 'icons', 'hicolor',
                            '@0@x@0@'.format(size), 'apps'),
  )
endforeach
install_data(
  join_paths('symbolic', 'apps', '@0@-symbolic.svg'.format(pps_namespace)),
  install_dir: join_paths(pps_datadir, 'icons', 'hicolor', 'symbolic', 'apps'),
  rename: '@0@-symbolic.svg'.format(app_id),
)
