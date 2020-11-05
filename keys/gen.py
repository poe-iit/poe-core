#!/usr/bin/env python3
from OpenSSL import crypto
from os import chmod
import stat

def generate_ssl_keys():
    digest = 'sha256'
    pkey = crypto.PKey()
    pkey.generate_key(crypto.TYPE_RSA, 2048)

    req = crypto.X509Req()
    subj = req.get_subject()
    setattr(subj, 'CN', 'Deluge Daemon')
    req.set_pubkey(pkey)
    req.sign(pkey, digest)

    cert = crypto.X509()
    cert.set_serial_number(54321)
    cert.set_version(2)
    cert.gmtime_adj_notBefore(0)
    cert.gmtime_adj_notAfter(60 * 60 * 24 * 365 * 3)
    cert.set_issuer(req.get_subject())
    cert.set_subject(req.get_subject())
    cert.set_pubkey(req.get_pubkey())
    cert.sign(pkey, digest)

    ext = crypto.X509Extension(b'subjectAltName', False, b'IP:127.0.0.1')
    cert.add_extensions([ext])

    return pkey, cert

if __name__ == '__main__':
    pkey, cert = generate_ssl_keys()

    pkey_path = 'key.pkey'
    cert_path = 'key.cert'

    with open(pkey_path, 'wb') as f:
        f.write(crypto.dump_privatekey(crypto.FILETYPE_PEM, pkey))

    with open(cert_path, 'wb') as f:
        f.write(crypto.dump_certificate(crypto.FILETYPE_PEM, cert))

    for path in (pkey_path, cert_path):
        chmod(path, stat.S_IREAD | stat.S_IWRITE)
