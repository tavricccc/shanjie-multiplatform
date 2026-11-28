"""Transfer release inputs without relying on the runner's native tar implementation."""
import argparse
import pathlib
import tarfile


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('pack', 'unpack'))
    parser.add_argument('archive', type=pathlib.Path)
    parser.add_argument('--root', type=pathlib.Path, required=True)
    parser.add_argument('--paths', nargs='+')
    args = parser.parse_args(argv)
    if args.operation == 'pack':
        if not args.paths:
            parser.error('pack requires --paths')
        with tarfile.open(args.archive, 'w', format=tarfile.PAX_FORMAT) as archive:
            for path in args.paths:
                archive.add(args.root / path, arcname=path)
    else:
        with tarfile.open(args.archive, 'r:') as archive:
            archive.extractall(args.root, filter='data')
    print(f'{args.operation}: {args.archive}')


if __name__ == '__main__':
    main()
