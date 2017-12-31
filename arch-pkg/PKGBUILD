# Maintainer: Hans Ole Hatzel <hhatzel@gmail.com>

pkgname=vorleser-server
pkgdesc='A server to serve audiobook files and synchronize playback positions.'
arch=('i686' 'x86_64')
url='https://github.com/hatzel/vorleser-server'
license=('MIT')
pkgver=0.0.1
pkgrel=1
depends=('systemd' 'ffmpeg' 'sqlite')
provides=('vorleser')
backup=('etc/vorleser.toml')
install='vorleser.install'
makedepends=('rust' 'cargo' 'git')
source=("$pkgname::git+https://github.com/hatzel/vorleser-server")
sha256sums=('SKIP')

pgkver() {
  git describe --long | sed -r 's/-([0-9,a-g,A-G]{7}.*)//' | sed 's/-/./'
}

package() {
  cd $pkgname
  cargo build --release

  install -D -m755 "$srcdir/$pkgname/target/release/vorleser_server_bin" "$pkgdir/usr/bin/vorleser"
  install -D -m644 "$srcdir/$pkgname/vorleser-default.toml" "$pkgdir/etc/vorleser.toml"
  install -D -m644 ../../vorleser.service "$pkgdir/usr/lib/systemd/system/vorleser.service"
  install -D -m644 ../../vorleser.sysuser "$pkgdir/usr/lib/sysusers.d/vorleser.conf"

  install -D -m644 "$srcdir/$pkgname/LICENSE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
}

# vim: ts=2 sw=2 et:
