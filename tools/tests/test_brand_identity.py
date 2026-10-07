#!/usr/bin/env python3
"""Focused source, native-size pixel and consumer checks for issue #91."""
import hashlib
import json
from pathlib import Path
import re
import unittest
import xml.etree.ElementTree as ET
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
BRAND = ROOT / 'assets/brand'
NS = {'s':'http://www.w3.org/2000/svg'}


class BrandIdentityTest(unittest.TestCase):
    def test_provenance_and_every_export_matches_recorded_source(self):
        receipt=json.loads((BRAND/'generated/exports.json').read_text())
        self.assertEqual(receipt['source_commit'],'ae40d8efda28c141127116b3342d80843aba4185')
        handoff=json.loads((BRAND/'provenance.json').read_text())
        self.assertEqual(handoff['source_commit'],receipt['source_commit'])
        for name,digest in handoff['source_sha256'].items():
            self.assertEqual(hashlib.sha256((BRAND/name).read_bytes()).hexdigest(),digest,name)
        for name, digest in receipt['canonical_sha256'].items():
            self.assertEqual(hashlib.sha256((BRAND/name).read_bytes()).hexdigest(),digest,name)
        for name,digest in receipt['outputs'].items():
            self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        self.assertGreater(len(receipt['outputs']),50)

    def test_canonical_vectors_are_real_outlined_paths(self):
        for source in BRAND.glob('*.svg'):
            root=ET.parse(source).getroot()
            self.assertTrue(root.findall('.//s:path',NS),source.name)
            for forbidden in ('image','text','filter','linearGradient','radialGradient'):
                self.assertFalse(root.findall(f'.//s:{forbidden}',NS),source.name)
            self.assertNotIn('data:image',source.read_text())
        mark=ET.parse(BRAND/'tabula-mark-primary.svg').getroot().find('s:path',NS)
        for variant in ('on-dark','mono','black','white'):
            other=ET.parse(BRAND/f'tabula-mark-{variant}.svg').getroot().find('s:path',NS)
            self.assertEqual(mark.attrib['d'],other.attrib['d'])

    def test_native_size_mark_alpha_and_portal_remain_open(self):
        for size in (16,24,32,48,64):
            with Image.open(BRAND/f'web/mark-{size}.png') as source:
                im=source.convert('RGBA')
                self.assertEqual(im.size,(size,size))
                self.assertEqual(im.getpixel((0,0))[3],0)
                # The open portal in the bottom quarter must separate both legs.
                y=round(size*.8)
                self.assertLess(im.getpixel((size//2,y))[3],32,(size,'portal'))
                self.assertGreater(im.getpixel((round(size*.38),y))[3],180,(size,'left leg'))
                self.assertGreater(im.getpixel((round(size*.62),y))[3],180,(size,'right leg'))
        with Image.open(BRAND/'web/favicon.ico') as icon:
            self.assertEqual(icon.ico.sizes(),{(16,16),(32,32),(48,48)})
            self.assertEqual(icon.ico.getimage((16,16)).convert('RGBA').tobytes(),Image.open(BRAND/'web/mark-16.png').convert('RGBA').tobytes())

    def test_native_launchers_are_square_opaque_and_unmasked(self):
        for size in (192,512):
            with Image.open(BRAND/f'web/app-icon-{size}.png') as im:
                rgba=im.convert('RGBA')
                for y in range(size):
                    for x in range(size):
                        red,green,blue,alpha=rgba.getpixel((x,y))
                        # The opaque ink background may fill the mask; only the
                        # lavender essential mark must fit the Web maskable circle.
                        if max(abs(red-94),abs(green-75),abs(blue-139)) > 32:
                            self.assertLessEqual((x+.5-size/2)**2+(y+.5-size/2)**2,(size*.4)**2,(size,'web safe circle'))
        for size in (16,32,64):
            data=(BRAND/f'generated/native-icon-{size}.rgba').read_bytes()
            self.assertEqual(len(data),size*size*4)
            im=Image.frombytes('RGBA',(size,size),data)
            self.assertEqual(im.getpixel((0,0)),(94,75,139,255))
        for density,size in (('mdpi',48),('hdpi',72),('xhdpi',96),('xxhdpi',144),('xxxhdpi',192)):
            directory=ROOT/f'apps/mobile/android/src/main/res/mipmap-{density}'
            with Image.open(directory/'ic_launcher.png') as im:
                self.assertEqual(im.size,(size,size))
                self.assertEqual(im.convert('RGBA').getextrema()[3],(255,255))
            with Image.open(directory/'ic_launcher_foreground.png') as im:
                self.assertEqual(im.size,(size*108//48,size*108//48))
                self.assertEqual(im.convert('RGBA').getpixel((0,0))[3],0)
                rgba=im.convert('RGBA')
                center=rgba.width/2
                safe_radius=rgba.width*33/108
                for y in range(rgba.height):
                    for x in range(rgba.width):
                        if rgba.getpixel((x,y))[3] > 16:
                            self.assertLessEqual((x+.5-center)**2+(y+.5-center)**2,safe_radius**2,(density,'adaptive safe circle'))
        catalog=ROOT/'apps/mobile/ios/TabulaApp/Assets.xcassets/AppIcon.appiconset'
        entries=json.loads((catalog/'Contents.json').read_text())['images']
        self.assertEqual(len(entries),18)
        for entry in entries:
            size=round(float(entry['size'].split('x')[0])*int(entry['scale'][:-1]))
            with Image.open(catalog/entry['filename']) as im:
                self.assertEqual(im.size,(size,size))
                self.assertEqual(im.mode,'RGB')
                self.assertEqual(im.getpixel((0,0)),(94,75,139))

    def test_single_shared_lockup_and_accessible_name(self):
        svg=(BRAND/'generated/tabula-lockup.svg').read_text().strip()
        self.assertIn('viewBox="0 0 705.276 256"',svg)
        self.assertIn('translate(294 77)',svg)
        self.assertIn('aria-hidden="true"',svg)
        self.assertNotIn('<title',svg)
        self.assertIn('var(--sys-color-brand-mark)',svg)
        self.assertIn('var(--sys-color-brand-wordmark)',svg)
        shell=(ROOT/'apps/web/src/views/parts.rs').read_text()
        self.assertIn('include_str!("../../../../assets/brand/generated/tabula-lockup.svg")',shell)
        self.assertNotIn('M11 12h18v5h-6v12h-6V17h-6z',shell)
        for name in ('index.html','werewolf.html','play.html','werewolf-play.html'):
            html=(ROOT/f'apps/game-client/web/{name}').read_text()
            marks=re.findall(r'<!-- tabula-brand:start -->(.*?)<!-- tabula-brand:end -->',html,re.S)
            self.assertEqual(marks,[svg],name)
            self.assertEqual(html.count('aria-label="Tabula"'),2 if name=='index.html' else 1,name)
            self.assertIn('brand/manifest.webmanifest',html)
        paths=(ROOT/'apps/mobile/shared/src/commonMain/kotlin/com/loveoverflow/tabula/mobile/design/TabulaBrandPaths.kt').read_text()
        self.assertIn(ET.parse(BRAND/'tabula-mark-primary.svg').getroot().find('s:path',NS).attrib['d'],paths)
        self.assertIn('LockupWidth = 705.276f',paths)
        # No logo motion is applied. Existing UI reduced-motion behavior is unchanged.
        self.assertNotRegex((ROOT/'apps/web/style/shell.scss').read_text(),r'\.brand[^{}]*\{[^}]*animation')
        shell_css=(ROOT/'apps/web/style/shell.scss').read_text()
        self.assertRegex(shell_css,r'\.shell-menu \.brand \{[^}]*min-width:0;')
        self.assertRegex(shell_css,r'\.shell-menu \.brand-lockup \{[^}]*max-width:100%;[^}]*height:auto;[^}]*aspect-ratio:705\.276 / 256;')

    def test_web_manifest_and_platform_identity_are_wired(self):
        manifest=json.loads((BRAND/'web/manifest.webmanifest').read_text())
        self.assertEqual(manifest['theme_color'],'#5E4B8B')
        self.assertEqual(manifest['background_color'],'#EBE6F7')
        self.assertEqual(manifest['start_url'],'../')
        self.assertEqual({v['sizes'] for v in manifest['icons']},{'192x192','512x512'})
        for item in manifest['icons']:
            self.assertEqual(item['purpose'],'any maskable')
        self.assertIn('data-target-path="brand"',(ROOT/'apps/web/index.html').read_text())
        android=(ROOT/'apps/mobile/android/src/main/AndroidManifest.xml').read_text()
        self.assertIn('android:icon="@mipmap/ic_launcher"',android)
        self.assertIn('android:roundIcon="@mipmap/ic_launcher"',android)
        project=(ROOT/'apps/mobile/ios/TabulaApp.xcodeproj/project.pbxproj').read_text()
        self.assertIn('Assets.xcassets',project)
        self.assertEqual(project.count('ASSETCATALOG_COMPILER_APPICON_NAME = AppIcon'),2)
        for name in ('main.rs','bin/werewolf.rs'):
            self.assertIn('icon: Some(tabula_game_client::brand_icon())',(ROOT/'apps/game-client/src'/name).read_text())


if __name__=='__main__':unittest.main()
