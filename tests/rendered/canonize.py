from __future__ import print_function

import shutil
import os


if __name__ == '__main__':
    for fn in os.listdir('.'):
        if fn.endswith('_diff.png'):
            os.remove(fn)
        elif fn.endswith('.png'):
            fn = fn.replace('.png', '')
            if fn.isdigit():
                shutil.copy(fn + '.png', fn + '_expected.png')
                os.remove(fn + '.png')
