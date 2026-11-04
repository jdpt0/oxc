# edge-cases/js-in-vue/handler-lone-statement.vue

## Option 1

`````json
{"printWidth":80}
`````

### Diff

`````diff
===================================================================
--- prettier
+++ oxfmt
@@ -2,9 +2,9 @@
   <button
     @click="foo?.bar;"
     @input="foo?.bar;"
     @change="foo()?.bar;"
-    @blur="foo?.()"
+    @blur="foo?.();"
     @focus=""
     @keyup=""
     @keydown=""
   ></button>

`````

### Actual (oxfmt)

`````vue
<template>
  <button
    @click="foo?.bar;"
    @input="foo?.bar;"
    @change="foo()?.bar;"
    @blur="foo?.();"
    @focus=""
    @keyup=""
    @keydown=""
  ></button>
</template>

`````

### Expected (prettier)

`````vue
<template>
  <button
    @click="foo?.bar;"
    @input="foo?.bar;"
    @change="foo()?.bar;"
    @blur="foo?.()"
    @focus=""
    @keyup=""
    @keydown=""
  ></button>
</template>

`````

## Option 2

`````json
{"printWidth":100,"vueIndentScriptAndStyle":true,"singleQuote":true,"semi":false}
`````

### Diff

`````diff
===================================================================
--- prettier
+++ oxfmt
@@ -2,9 +2,9 @@
   <button
     @click="foo?.bar;"
     @input="foo?.bar;"
     @change="foo()?.bar;"
-    @blur="foo?.()"
+    @blur="foo?.();"
     @focus=""
     @keyup=""
     @keydown=""
   ></button>

`````

### Actual (oxfmt)

`````vue
<template>
  <button
    @click="foo?.bar;"
    @input="foo?.bar;"
    @change="foo()?.bar;"
    @blur="foo?.();"
    @focus=""
    @keyup=""
    @keydown=""
  ></button>
</template>

`````

### Expected (prettier)

`````vue
<template>
  <button
    @click="foo?.bar;"
    @input="foo?.bar;"
    @change="foo()?.bar;"
    @blur="foo?.()"
    @focus=""
    @keyup=""
    @keydown=""
  ></button>
</template>

`````
